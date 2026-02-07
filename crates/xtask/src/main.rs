use anyhow::{bail, Context as _, Result};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use sysinfo::{Pid, System};
use xnote_core::knowledge::{KnowledgeIndex, SearchOptions};
use xnote_core::vault::Vault;
use xnote_core::watch::VaultWatchChange;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        print_help();
        return Ok(());
    };

    match cmd.as_str() {
        "gen-vault" => cmd_gen_vault(args.collect()),
        "perf" => cmd_perf(args.collect()),
        "foundation-gate" => cmd_foundation_gate(args.collect()),
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
  perf        Print basic perf metrics (open/scan/index/search/watch)
  foundation-gate  Run full baseline gate (tests/check/perf profiles)

Examples:
  cargo run -p xtask -- gen-vault --path .\\Knowledge.vault --notes 100000 --max-depth 200 --clean
  cargo run -p xtask -- perf --path .\\Knowledge.vault --query n0
  cargo run -p xtask -- foundation-gate --path .\\Knowledge.vault --query note --iterations 10

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
        fs::remove_dir_all(&args.path)
            .with_context(|| format!("remove_dir_all: {}", args.path.display()))?;
    }

    fs::create_dir_all(&args.path)
        .with_context(|| format!("create_dir_all: {}", args.path.display()))?;
    fs::create_dir_all(args.path.join(".xnote").join("order"))
        .with_context(|| "create .xnote/order")?;
    fs::create_dir_all(args.path.join(".xnote").join("cache"))
        .with_context(|| "create .xnote/cache")?;

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
        fs::create_dir_all(&fs_path)
            .with_context(|| format!("create_dir_all: {}", fs_path.display()))?;
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
            args.content_min_bytes
                + rng.gen_range_usize(args.content_max_bytes - args.content_min_bytes + 1)
        };
        let with_link = rng.gen_bool_percent(args.link_percent);
        let content = gen_markdown_content(&mut rng, i, args.notes, size, with_link);

        fs::write(&full_path, content)
            .with_context(|| format!("write: {}", full_path.display()))?;
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
            "--max-depth" => {
                max_depth = it.next().context("--max-depth requires a value")?.parse()?
            }
            "--seed" => seed = it.next().context("--seed requires a value")?.parse()?,
            "--clean" => clean = true,
            "--content-min" => {
                content_min_bytes = it
                    .next()
                    .context("--content-min requires a value")?
                    .parse()?
            }
            "--content-max" => {
                content_max_bytes = it
                    .next()
                    .context("--content-max requires a value")?
                    .parse()?
            }
            "--link-percent" => {
                link_percent = it
                    .next()
                    .context("--link-percent requires a value")?
                    .parse()?
            }
            "--order-take" => {
                order_take = it
                    .next()
                    .context("--order-take requires a value")?
                    .parse()?
            }
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

fn gen_markdown_content(
    rng: &mut Rng,
    ix: usize,
    total: usize,
    target_bytes: usize,
    with_link: bool,
) -> String {
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
    query: String,
    iterations: usize,
}

fn cmd_perf(args: Vec<String>) -> Result<()> {
    let args = parse_perf_args(args)?;

    let open_start = Instant::now();
    let vault = Vault::open(&args.path)?;
    let open_ms = open_start.elapsed().as_millis();

    let scan_start = Instant::now();
    let entries = vault.fast_scan_notes()?;
    let scan_ms = scan_start.elapsed().as_millis();

    let index_start = Instant::now();
    let mut by_folder: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in &entries {
        let folder = match e.path.rsplit_once('/') {
            Some((folder, _)) => folder.to_string(),
            None => String::new(),
        };
        by_folder.entry(folder).or_default().push(e.path.clone());
    }
    for paths in by_folder.values_mut() {
        paths.sort();
    }

    let mut child_sets: HashMap<String, HashSet<String>> = HashMap::new();
    child_sets.entry(String::new()).or_default();
    let folder_keys: Vec<String> = by_folder.keys().cloned().collect();
    for folder in folder_keys {
        if folder.is_empty() {
            continue;
        }

        let mut full = folder;
        loop {
            let parent = match full.rsplit_once('/') {
                Some((p, _)) => p.to_string(),
                None => String::new(),
            };
            child_sets
                .entry(parent.clone())
                .or_default()
                .insert(full.clone());
            if parent.is_empty() {
                break;
            }
            full = parent;
        }
    }
    let index_ms = index_start.elapsed().as_millis();

    let order_start = Instant::now();
    let mut orders: HashMap<String, Vec<String>> = HashMap::new();
    let mut total_order_entries = 0usize;
    for folder in by_folder.keys().filter(|f| !f.is_empty()) {
        let order = vault.load_folder_order(folder)?;
        total_order_entries += order.len();
        orders.insert(folder.clone(), order);
    }
    let order_ms = order_start.elapsed().as_millis();

    let apply_start = Instant::now();
    let mut total_ordered_notes = 0usize;
    for (folder, default_paths) in by_folder.iter_mut() {
        let ordered_paths = if folder.is_empty() {
            default_paths.clone()
        } else {
            let order = orders.get(folder).map(|v| v.as_slice()).unwrap_or(&[]);
            apply_folder_order(default_paths, order)
        };
        total_ordered_notes += ordered_paths.len();
        *default_paths = ordered_paths;
    }
    let apply_ms = apply_start.elapsed().as_millis();

    let read_ms = if let Some(first) = entries.first() {
        let read_start = Instant::now();
        let _content = vault.read_note(&first.path)?;
        read_start.elapsed().as_millis()
    } else {
        0
    };

    let filter_lower_start = Instant::now();
    let paths_lower: Vec<String> = entries.iter().map(|e| e.path.to_lowercase()).collect();
    let lower_ms = filter_lower_start.elapsed().as_millis();

    let query = args.query.trim().to_lowercase();
    let filter_start = Instant::now();
    let mut filter_matches = 0usize;
    if !query.is_empty() {
        for p in &paths_lower {
            if p.contains(&query) {
                filter_matches += 1;
            }
        }
    }
    let filter_ms = filter_start.elapsed().as_millis();

    let knowledge_build_start = Instant::now();
    let mut knowledge_index = KnowledgeIndex::build_from_entries(&vault, &entries)?;
    let knowledge_build_ms = knowledge_build_start.elapsed().as_millis();

    let mut search_samples = Vec::with_capacity(args.iterations);
    let mut quick_open_samples = Vec::with_capacity(args.iterations);
    let mut watch_apply_samples = Vec::with_capacity(args.iterations);
    let search_options = SearchOptions::default();

    for _ in 0..args.iterations {
        let search_start = Instant::now();
        let _ = knowledge_index.search(&vault, &query, search_options.clone());
        search_samples.push(search_start.elapsed().as_millis());

        let quick_open_start = Instant::now();
        let _ = knowledge_index.quick_open_paths(&query, 200);
        quick_open_samples.push(quick_open_start.elapsed().as_millis());
    }

    let watch_target = entries
        .first()
        .map(|entry| entry.path.clone())
        .unwrap_or_else(|| "notes/sample.md".to_string());
    let watch_changes = vec![
        VaultWatchChange::NoteChanged {
            path: watch_target.clone(),
        },
        VaultWatchChange::NoteChanged {
            path: watch_target.clone(),
        },
    ];

    let watch_base = vault
        .read_note(&watch_target)
        .unwrap_or_else(|_| "# WatchSample\ncontent\n".to_string());
    let watch_restore = watch_base.clone();
    let watch_patch = format!("{watch_base}\nwatch-perf-marker\n");

    if entries.first().is_some() {
        for _ in 0..args.iterations {
            vault.write_note(&watch_target, &watch_patch)?;

            let watch_start = Instant::now();
            apply_watch_changes_benchmark(&mut knowledge_index, &vault, watch_changes.clone())?;
            watch_apply_samples.push(watch_start.elapsed().as_millis());

            vault.write_note(&watch_target, &watch_restore)?;
        }
        let _ = knowledge_index.upsert_note(&vault, &watch_target);
    }

    println!("perf:");
    println!("  path: {}", args.path.display());
    println!("  open_ms: {open_ms}");
    println!("  scan_ms: {scan_ms}");
    println!("  index_ms: {index_ms}");
    println!("  knowledge_index_build_ms: {knowledge_build_ms}");
    println!("  note_count: {}", entries.len());
    println!("  folder_count_notes: {}", by_folder.len());
    println!("  folder_count_tree: {}", child_sets.len());
    println!("  order_load_ms: {order_ms}");
    println!("  order_apply_ms: {apply_ms}");
    println!("  order_total_entries: {total_order_entries}");
    println!("  order_total_notes_after_apply: {total_ordered_notes}");
    println!("  read_first_note_ms: {read_ms}");
    println!("  filter_lower_ms: {lower_ms}");
    println!("  filter_query: {query}");
    println!("  filter_query_ms: {filter_ms}");
    println!("  filter_matches: {filter_matches}");
    if let Some((rss_kb, vmem_kb)) = current_process_memory_kb() {
        println!("  rss_kb: {rss_kb}");
        println!("  vmem_kb: {vmem_kb}");
    } else {
        println!("  rss_kb: N/A");
        println!("  vmem_kb: N/A");
    }
    if !search_samples.is_empty() {
        let p50 = percentile_ms(&search_samples, 50.0);
        let p95 = percentile_ms(&search_samples, 95.0);
        println!("  search_samples: {}", search_samples.len());
        println!("  search_p50_ms: {p50}");
        println!("  search_p95_ms: {p95}");
    }
    if !quick_open_samples.is_empty() {
        let p50 = percentile_ms(&quick_open_samples, 50.0);
        let p95 = percentile_ms(&quick_open_samples, 95.0);
        println!("  quick_open_samples: {}", quick_open_samples.len());
        println!("  quick_open_p50_ms: {p50}");
        println!("  quick_open_p95_ms: {p95}");
    }
    if !watch_apply_samples.is_empty() {
        let p50 = percentile_ms(&watch_apply_samples, 50.0);
        let p95 = percentile_ms(&watch_apply_samples, 95.0);
        println!("  watch_apply_samples: {}", watch_apply_samples.len());
        println!("  watch_apply_p50_ms: {p50}");
        println!("  watch_apply_p95_ms: {p95}");
    }

    Ok(())
}

fn parse_perf_args(args: Vec<String>) -> Result<PerfArgs> {
    let mut path: Option<PathBuf> = None;
    let mut query: String = "note".to_string();
    let mut iterations: usize = 20;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--path" | "--vault" => {
                path = Some(PathBuf::from(it.next().context("--path requires a value")?))
            }
            "--query" => query = it.next().context("--query requires a value")?,
            "--iterations" => {
                let raw = it.next().context("--iterations requires a value")?;
                iterations = raw
                    .parse::<usize>()
                    .with_context(|| format!("invalid --iterations: {raw}"))?;
            }
            other => bail!("unknown perf arg: {other}"),
        }
    }
    Ok(PerfArgs {
        path: path.unwrap_or_else(|| PathBuf::from("Knowledge.vault")),
        query,
        iterations: iterations.max(1),
    })
}

struct FoundationGateArgs {
    path: PathBuf,
    query: String,
    iterations: usize,
}

fn cmd_foundation_gate(args: Vec<String>) -> Result<()> {
    let args = parse_foundation_gate_args(args)?;
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .context("resolve workspace root")?;

    run_command_step(
        "core-tests",
        &workspace_root,
        "cargo",
        &["test", "-p", "xnote-core"],
    )?;
    run_command_step(
        "ui-check",
        &workspace_root,
        "cargo",
        &["check", "-p", "xnote-ui"],
    )?;
    run_command_step(
        "ui-tests-compile",
        &workspace_root,
        "cargo",
        &["test", "-p", "xnote-ui", "--no-run"],
    )?;
    run_command_step(
        "xtask-check",
        &workspace_root,
        "cargo",
        &["check", "-p", "xtask"],
    )?;

    let vault = args.path.to_string_lossy().to_string();
    let iterations = args.iterations.to_string();

    let perf_default_args = vec![
        "scripts/check_perf_baseline.py".to_string(),
        "--vault".to_string(),
        vault.clone(),
        "--query".to_string(),
        args.query.clone(),
        "--iterations".to_string(),
        iterations.clone(),
        "--retries".to_string(),
        "3".to_string(),
        "--baseline-profile".to_string(),
        "default".to_string(),
        "--report-out".to_string(),
        "perf/latest-report.json".to_string(),
        "--previous-report".to_string(),
        "perf/latest-report.json".to_string(),
        "--delta-report-out".to_string(),
        "perf/latest-delta-report.json".to_string(),
    ];
    let perf_default_args_ref = perf_default_args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    run_command_step(
        "perf-default-profile",
        &workspace_root,
        "python",
        &perf_default_args_ref,
    )?;

    let perf_windows_ci_args = vec![
        "scripts/check_perf_baseline.py".to_string(),
        "--vault".to_string(),
        vault,
        "--query".to_string(),
        args.query,
        "--iterations".to_string(),
        iterations,
        "--retries".to_string(),
        "3".to_string(),
        "--baseline-profile".to_string(),
        "windows_ci".to_string(),
        "--report-out".to_string(),
        "perf/latest-report-windows-ci.json".to_string(),
        "--previous-report".to_string(),
        "perf/latest-report-windows-ci.json".to_string(),
        "--delta-report-out".to_string(),
        "perf/latest-delta-report-windows-ci.json".to_string(),
    ];
    let perf_windows_ci_args_ref = perf_windows_ci_args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    run_command_step(
        "perf-windows-ci-profile",
        &workspace_root,
        "python",
        &perf_windows_ci_args_ref,
    )?;

    eprintln!("foundation-gate: OK");
    Ok(())
}

fn parse_foundation_gate_args(args: Vec<String>) -> Result<FoundationGateArgs> {
    let mut path: Option<PathBuf> = None;
    let mut query: String = "note".to_string();
    let mut iterations: usize = 10;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--path" | "--vault" => {
                path = Some(PathBuf::from(it.next().context("--path requires a value")?))
            }
            "--query" => query = it.next().context("--query requires a value")?,
            "--iterations" => {
                let raw = it.next().context("--iterations requires a value")?;
                iterations = raw
                    .parse::<usize>()
                    .with_context(|| format!("invalid --iterations: {raw}"))?;
            }
            other => bail!("unknown foundation-gate arg: {other}"),
        }
    }

    Ok(FoundationGateArgs {
        path: path.unwrap_or_else(|| PathBuf::from("Knowledge.vault")),
        query,
        iterations: iterations.max(1),
    })
}

fn run_command_step(name: &str, cwd: &std::path::Path, program: &str, args: &[&str]) -> Result<()> {
    eprintln!("\n==> [{name}] {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .with_context(|| format!("spawn step failed: {name}"))?;
    if !status.success() {
        bail!("step failed: {name}");
    }
    Ok(())
}

fn apply_watch_changes_benchmark(
    index: &mut KnowledgeIndex,
    vault: &Vault,
    changes: Vec<VaultWatchChange>,
) -> Result<()> {
    for change in changes {
        match change {
            VaultWatchChange::NoteChanged { path } => {
                if vault.read_note(&path).is_ok() {
                    index.upsert_note(vault, &path)?;
                }
            }
            VaultWatchChange::NoteRemoved { path } => index.remove_note(&path),
            VaultWatchChange::NoteMoved { from, to } => {
                index.remove_note(&from);
                if vault.read_note(&to).is_ok() {
                    index.upsert_note(vault, &to)?;
                }
            }
            VaultWatchChange::FolderCreated { .. }
            | VaultWatchChange::FolderRemoved { .. }
            | VaultWatchChange::FolderMoved { .. } => {
                *index = KnowledgeIndex::rebuild_from_vault(vault)?;
            }
            VaultWatchChange::RescanRequired => {
                *index = KnowledgeIndex::rebuild_from_vault(vault)?;
            }
        }
    }
    Ok(())
}

fn percentile_ms(samples: &[u128], percentile: f64) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();

    let rank = ((percentile / 100.0) * ((sorted.len() - 1) as f64)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn apply_folder_order(default_paths: &[String], order: &[String]) -> Vec<String> {
    let existing: HashSet<&str> = default_paths.iter().map(|s| s.as_str()).collect();
    let mut out = Vec::with_capacity(default_paths.len());
    let mut seen: HashSet<&str> = HashSet::with_capacity(default_paths.len());

    for p in order {
        let p = p.as_str();
        if existing.contains(p) && seen.insert(p) {
            out.push(p.to_string());
        }
    }
    for p in default_paths {
        let p = p.as_str();
        if seen.insert(p) {
            out.push(p.to_string());
        }
    }

    out
}

fn current_process_memory_kb() -> Option<(u64, u64)> {
    let mut system = System::new();
    system.refresh_processes();
    let pid = Pid::from_u32(std::process::id());
    let process = system.process(pid)?;
    Some((process.memory(), process.virtual_memory()))
}
