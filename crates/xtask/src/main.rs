use anyhow::{Context as _, Result, bail};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use xnote_core::vault::Vault;

fn main() -> Result<()> {
  let mut args = std::env::args().skip(1);
  let Some(cmd) = args.next() else {
    print_help();
    return Ok(());
  };

  match cmd.as_str() {
    "gen-vault" => cmd_gen_vault(args.collect()),
    "perf" => cmd_perf(args.collect()),
    "help" | "-h" | "--help" => {
      print_help();
      Ok(())
    }
    other => {
      print_help();
      bail!("unknown xtask command: {other}");
    }
  }
}

fn print_help() {
  eprintln!(
    r#"xtask (XNote)

Commands:
  gen-vault   Generate a large Knowledge vault dataset
  perf        Print basic perf metrics (open/scan/order/read)

Examples:
  cargo run -p xtask -- gen-vault --path .\\Knowledge.vault --notes 100000 --max-depth 200 --clean
  cargo run -p xtask -- perf --path .\\Knowledge.vault

XNote UI:
  $env:XNOTE_VAULT = "C:\path\to\Knowledge.vault"
  cargo run -p xnote-ui
"#
  );
}

#[derive(Clone)]
struct Rng(u64);

impl Rng {
  fn new(seed: u64) -> Self {
    Self(seed)
  }

  fn next_u64(&mut self) -> u64 {
    // splitmix64
    self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = self.0;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
  }

  fn next_u32(&mut self) -> u32 {
    (self.next_u64() >> 32) as u32
  }

  fn gen_range_usize(&mut self, max_exclusive: usize) -> usize {
    if max_exclusive == 0 {
      return 0;
    }
    (self.next_u64() as usize) % max_exclusive
  }

  fn gen_bool_percent(&mut self, percent: u32) -> bool {
    (self.next_u32() % 100) < percent
  }
}

struct GenVaultArgs {
  path: PathBuf,
  notes: usize,
  max_depth: usize,
  seed: u64,
  clean: bool,
  content_min_bytes: usize,
  content_max_bytes: usize,
  link_percent: u32,
  order_take: usize,
}

fn cmd_gen_vault(args: Vec<String>) -> Result<()> {
  let args = parse_gen_vault_args(args)?;

  if args.clean && args.path.exists() {
    fs::remove_dir_all(&args.path).with_context(|| format!("remove_dir_all: {}", args.path.display()))?;
  }

  fs::create_dir_all(&args.path).with_context(|| format!("create_dir_all: {}", args.path.display()))?;
  fs::create_dir_all(args.path.join(".xnote").join("order")).with_context(|| "create .xnote/order")?;
  fs::create_dir_all(args.path.join(".xnote").join("cache")).with_context(|| "create .xnote/cache")?;

  let notes_root = args.path.join("notes");
  fs::create_dir_all(&notes_root).with_context(|| "create notes/")?;

  let mut folders: Vec<(String, PathBuf, Vec<String>)> = Vec::with_capacity(args.max_depth + 1);
  let mut rel = "notes".to_string();
  let mut fs_path = notes_root.clone();
  folders.push((rel.clone(), fs_path.clone(), Vec::new()));

  for depth in 1..=args.max_depth {
    let seg = format!("d{:03}", depth - 1);
    rel.push('/');
    rel.push_str(&seg);
    fs_path.push(&seg);
    fs::create_dir_all(&fs_path).with_context(|| format!("create_dir_all: {}", fs_path.display()))?;
    folders.push((rel.clone(), fs_path.clone(), Vec::new()));
  }

  let mut rng = Rng::new(args.seed);
  let started = Instant::now();

  for i in 0..args.notes {
    let depth = gen_depth(&mut rng, args.max_depth);
    let name = gen_note_name(&mut rng, i);
    let file_name = format!("{name}.md");
    let full_path = folders[depth].1.join(&file_name);
    let rel_posix = format!("{}/{}", folders[depth].0, file_name);

    let size = if args.content_max_bytes <= args.content_min_bytes {
      args.content_min_bytes
    } else {
      args.content_min_bytes + rng.gen_range_usize(args.content_max_bytes - args.content_min_bytes + 1)
    };
    let with_link = rng.gen_bool_percent(args.link_percent);
    let content = gen_markdown_content(&mut rng, i, args.notes, size, with_link);

    fs::write(&full_path, content).with_context(|| format!("write: {}", full_path.display()))?;
    folders[depth].2.push(rel_posix);

    if i > 0 && i % 10_000 == 0 {
      eprintln!("gen-vault: wrote {i} notes...");
    }
  }

  let gen_ms = started.elapsed().as_millis();

  // Write some order files to exercise load/apply.
  let vault = Vault::open(&args.path)?;
  for (folder, _fs_path, note_paths) in &mut folders {
    if note_paths.len() < 2 {
      continue;
    }
    let take = args.order_take.min(note_paths.len());
    if take < 2 {
      continue;
    }

    let mut ordered = note_paths[..take].to_vec();
    shuffle(&mut rng, &mut ordered);
    vault.save_folder_order(folder, &ordered)?;
  }

  eprintln!(
    "gen-vault: done\n  path: {}\n  notes: {}\n  max_depth: {}\n  time_ms: {}",
    args.path.display(),
    args.notes,
    args.max_depth,
    gen_ms
  );
  eprintln!(
    "gen-vault: run UI with\n  $env:XNOTE_VAULT = \"{}\"\n  cargo run -p xnote-ui",
    args.path.display()
  );

  Ok(())
}

fn parse_gen_vault_args(args: Vec<String>) -> Result<GenVaultArgs> {
  let mut path: Option<PathBuf> = None;
  let mut notes: usize = 100_000;
  let mut max_depth: usize = 200;
  let mut seed: u64 = 1;
  let mut clean = false;
  let mut content_min_bytes: usize = 1024;
  let mut content_max_bytes: usize = 4096;
  let mut link_percent: u32 = 10;
  let mut order_take: usize = 32;

  let mut it = args.into_iter();
  while let Some(arg) = it.next() {
    match arg.as_str() {
      "--path" => path = Some(PathBuf::from(it.next().context("--path requires a value")?)),
      "--notes" => notes = it.next().context("--notes requires a value")?.parse()?,
      "--max-depth" => max_depth = it.next().context("--max-depth requires a value")?.parse()?,
      "--seed" => seed = it.next().context("--seed requires a value")?.parse()?,
      "--clean" => clean = true,
      "--content-min" => content_min_bytes = it.next().context("--content-min requires a value")?.parse()?,
      "--content-max" => content_max_bytes = it.next().context("--content-max requires a value")?.parse()?,
      "--link-percent" => link_percent = it.next().context("--link-percent requires a value")?.parse()?,
      "--order-take" => order_take = it.next().context("--order-take requires a value")?.parse()?,
      other => bail!("unknown gen-vault arg: {other}"),
    }
  }

  let path = path.unwrap_or_else(|| PathBuf::from("Knowledge.vault"));
  Ok(GenVaultArgs {
    path,
    notes,
    max_depth,
    seed,
    clean,
    content_min_bytes,
    content_max_bytes,
    link_percent,
    order_take,
  })
}

fn gen_depth(rng: &mut Rng, max_depth: usize) -> usize {
  // Geometric-ish distribution biased toward shallow paths.
  let mut depth = 0usize;
  while depth < max_depth && (rng.next_u32() & 0b111) == 0 {
    depth += 1;
  }
  depth
}

fn gen_note_name(rng: &mut Rng, ix: usize) -> String {
  // Keep filenames readable + deterministic: n000123_ab12cd34ef
  let mut s = format!("n{ix:06}_");
  s.push_str(&rand_ascii(rng, 10));
  s
}

fn rand_ascii(rng: &mut Rng, len: usize) -> String {
  const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
  let mut out = String::with_capacity(len);
  for _ in 0..len {
    let ix = rng.gen_range_usize(ALPHABET.len());
    out.push(ALPHABET[ix] as char);
  }
  out
}

fn gen_markdown_content(rng: &mut Rng, ix: usize, total: usize, target_bytes: usize, with_link: bool) -> String {
  let mut s = String::new();
  s.push_str(&format!("# Note {ix:06}\n\n"));

  if with_link && total > 0 {
    let target = rng.gen_range_usize(total);
    s.push_str(&format!("[[Note {target:06}]]\n\n"));
  }

  while s.len() < target_bytes {
    s.push_str("Lorem ipsum dolor sit amet, consectetur adipiscing elit.\n");
    s.push_str("- Item A\n- Item B\n- Item C\n\n");
  }

  // Ensure ASCII-only content so truncation stays valid UTF-8.
  s.truncate(target_bytes);
  s.push('\n');
  s
}

fn shuffle(rng: &mut Rng, items: &mut [String]) {
  for i in (1..items.len()).rev() {
    let j = rng.gen_range_usize(i + 1);
    items.swap(i, j);
  }
}

struct PerfArgs {
  path: PathBuf,
}

fn cmd_perf(args: Vec<String>) -> Result<()> {
  let args = parse_perf_args(args)?;

  let open_start = Instant::now();
  let vault = Vault::open(&args.path)?;
  let open_ms = open_start.elapsed().as_millis();

  let scan_start = Instant::now();
  let entries = vault.fast_scan_notes()?;
  let scan_ms = scan_start.elapsed().as_millis();

  let mut folders = HashSet::<String>::new();
  for e in &entries {
    let folder = match e.path.rsplit_once('/') {
      Some((folder, _)) => folder.to_string(),
      None => String::new(),
    };
    folders.insert(folder);
  }

  let order_start = Instant::now();
  let mut total_order_entries = 0usize;
  for folder in folders.iter().filter(|f| !f.is_empty()) {
    let order = vault.load_folder_order(folder)?;
    total_order_entries += order.len();
  }
  let order_ms = order_start.elapsed().as_millis();

  let read_ms = if let Some(first) = entries.first() {
    let read_start = Instant::now();
    let _content = vault.read_note(&first.path)?;
    read_start.elapsed().as_millis()
  } else {
    0
  };

  println!("perf:");
  println!("  path: {}", args.path.display());
  println!("  open_ms: {open_ms}");
  println!("  scan_ms: {scan_ms}");
  println!("  note_count: {}", entries.len());
  println!("  folder_count: {}", folders.len());
  println!("  order_load_ms: {order_ms}");
  println!("  order_total_entries: {total_order_entries}");
  println!("  read_first_note_ms: {read_ms}");
  println!("  search_ms: TODO (SEARCH-001 not implemented)");

  Ok(())
}

fn parse_perf_args(args: Vec<String>) -> Result<PerfArgs> {
  let mut path: Option<PathBuf> = None;
  let mut it = args.into_iter();
  while let Some(arg) = it.next() {
    match arg.as_str() {
      "--path" | "--vault" => path = Some(PathBuf::from(it.next().context("--path requires a value")?)),
      other => bail!("unknown perf arg: {other}"),
    }
  }
  Ok(PerfArgs {
    path: path.unwrap_or_else(|| PathBuf::from("Knowledge.vault")),
  })
}
