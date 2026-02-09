use anyhow::{bail, Context as _, Result};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use sysinfo::{Pid, System};
use xnote_core::knowledge::{KnowledgeIndex, SearchOptions};
use xnote_core::vault::Vault;
use xnote_core::watch::{
    collapse_move_pairs, expand_folder_move_pairs_to_note_moves, note_path_has_folder_prefix,
    VaultWatchChange,
};

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
    let watch_txn_sample_count = args.iterations.min(3).max(1);
    let mut watch_txn_apply_samples = Vec::with_capacity(watch_txn_sample_count);
    let mut watch_txn_rebuild_samples = Vec::with_capacity(watch_txn_sample_count);
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
            let _ =
                apply_watch_changes_benchmark(&mut knowledge_index, &vault, watch_changes.clone())?;
            watch_apply_samples.push(watch_start.elapsed().as_millis());

            vault.write_note(&watch_target, &watch_restore)?;
        }
        let _ = knowledge_index.upsert_note(&vault, &watch_target);
    }

    let watch_txn_batch_dirs = 1_024usize;
    let watch_txn_changes = synthesize_watch_txn_replay_changes(
        &entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>(),
        watch_txn_batch_dirs,
    );
    let mut watch_txn_index = knowledge_index.clone();
    for _ in 0..watch_txn_sample_count {
        let watch_txn_started = Instant::now();
        let watch_txn_stats =
            apply_watch_changes_benchmark(&mut watch_txn_index, &vault, watch_txn_changes.clone())?;
        watch_txn_apply_samples.push(watch_txn_started.elapsed().as_millis());
        watch_txn_rebuild_samples.push(watch_txn_stats.rebuild_count as u128);
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
    println!("  watch_txn_batch_dirs: {watch_txn_batch_dirs}");
    println!("  watch_txn_batch_changes: {}", watch_txn_changes.len());
    if !watch_txn_apply_samples.is_empty() {
        let p50 = percentile_ms(&watch_txn_apply_samples, 50.0);
        let p95 = percentile_ms(&watch_txn_apply_samples, 95.0);
        println!(
            "  watch_txn_apply_samples: {}",
            watch_txn_apply_samples.len()
        );
        println!("  watch_txn_apply_ms: {p50}");
        println!("  watch_txn_apply_p50_ms: {p50}");
        println!("  watch_txn_apply_p95_ms: {p95}");
    }
    if !watch_txn_rebuild_samples.is_empty() {
        let p95 = percentile_ms(&watch_txn_rebuild_samples, 95.0);
        println!("  watch_txn_rebuild_p95: {p95}");
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

fn synthesize_watch_txn_replay_changes(
    existing_note_paths: &[String],
    batch_dirs: usize,
) -> Vec<VaultWatchChange> {
    let dir_count = batch_dirs.max(1_024);
    let note_sample_count = existing_note_paths.len().min(256);
    let mut changes = Vec::with_capacity(dir_count * 4 + note_sample_count);

    for i in 0..dir_count {
        let source = format!("notes/__watch_txn_batch/d{i:04}");
        let stage = format!("notes/__watch_txn_batch_stage/d{i:04}");
        let target = format!("notes/__watch_txn_batch_final/d{i:04}");

        changes.push(VaultWatchChange::FolderCreated {
            path: source.clone(),
        });
        changes.push(VaultWatchChange::FolderMoved {
            from: source,
            to: stage.clone(),
        });
        changes.push(VaultWatchChange::FolderMoved {
            from: stage,
            to: target.clone(),
        });

        if i % 6 == 0 {
            changes.push(VaultWatchChange::FolderRemoved { path: target });
        }

        if note_sample_count > 0 {
            changes.push(VaultWatchChange::NoteChanged {
                path: existing_note_paths[i % note_sample_count].clone(),
            });
        }
    }

    changes
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct WatchApplyBenchmarkStats {
    note_upsert_count: usize,
    note_remove_count: usize,
    note_move_count: usize,
    rebuild_count: usize,
}

fn apply_watch_changes_benchmark(
    index: &mut KnowledgeIndex,
    vault: &Vault,
    changes: Vec<VaultWatchChange>,
) -> Result<WatchApplyBenchmarkStats> {
    let mut stats = WatchApplyBenchmarkStats::default();
    let mut needs_rebuild = false;
    let mut note_changed_set: HashSet<String> = HashSet::new();
    let mut note_removed_set: HashSet<String> = HashSet::new();
    let mut note_moves: HashMap<String, String> = HashMap::new();
    let mut folder_removed_set: HashSet<String> = HashSet::new();
    let mut folder_moves: HashMap<String, String> = HashMap::new();

    for change in changes {
        match change {
            VaultWatchChange::NoteChanged { path } => {
                note_changed_set.insert(path);
            }
            VaultWatchChange::NoteRemoved { path } => {
                note_removed_set.insert(path);
            }
            VaultWatchChange::NoteMoved { from, to } => {
                if from != to {
                    note_moves.insert(from, to);
                }
            }
            VaultWatchChange::FolderCreated { .. } => {}
            VaultWatchChange::FolderRemoved { path } => {
                folder_removed_set.insert(path);
            }
            VaultWatchChange::FolderMoved { from, to } => {
                if from != to {
                    folder_moves.insert(from, to);
                }
            }
            VaultWatchChange::RescanRequired => {
                needs_rebuild = true;
                break;
            }
        }
    }

    if needs_rebuild {
        *index = KnowledgeIndex::rebuild_from_vault(vault)?;
        stats.rebuild_count = 1;
        return Ok(stats);
    }

    let note_moves_vec = match collapse_move_pairs(&note_moves.into_iter().collect::<Vec<_>>()) {
        Some(moves) => moves,
        None => {
            *index = KnowledgeIndex::rebuild_from_vault(vault)?;
            stats.rebuild_count = 1;
            return Ok(stats);
        }
    };

    let folder_moves_vec = match collapse_move_pairs(&folder_moves.into_iter().collect::<Vec<_>>())
    {
        Some(moves) => moves,
        None => {
            *index = KnowledgeIndex::rebuild_from_vault(vault)?;
            stats.rebuild_count = 1;
            return Ok(stats);
        }
    };

    let mut folder_removed_vec = folder_removed_set.into_iter().collect::<Vec<_>>();
    folder_removed_vec.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));

    let existing_paths = index.all_paths_sorted();

    let has_relevant_folder_move = folder_moves_vec.iter().any(|(from, _)| {
        existing_paths
            .iter()
            .any(|path| note_path_has_folder_prefix(path, from))
    });
    let has_relevant_folder_remove = folder_removed_vec.iter().any(|prefix| {
        existing_paths
            .iter()
            .any(|path| note_path_has_folder_prefix(path, prefix))
    });

    let folder_expanded_note_moves = if has_relevant_folder_move {
        match expand_folder_move_pairs_to_note_moves(&existing_paths, &folder_moves_vec) {
            Some(moves) => moves.into_iter().collect::<HashMap<_, _>>(),
            None => {
                *index = KnowledgeIndex::rebuild_from_vault(vault)?;
                stats.rebuild_count = 1;
                return Ok(stats);
            }
        }
    } else {
        HashMap::new()
    };

    if has_relevant_folder_move || has_relevant_folder_remove {
        for old_path in existing_paths {
            let moved_path = folder_expanded_note_moves
                .get(&old_path)
                .cloned()
                .unwrap_or_else(|| old_path.clone());

            let removed_by_folder = folder_removed_vec
                .iter()
                .any(|prefix| note_path_has_folder_prefix(&moved_path, prefix));
            if removed_by_folder {
                index.remove_note(&old_path);
                stats.note_remove_count += 1;
                continue;
            }

            if moved_path != old_path {
                index.remove_note(&old_path);
                stats.note_remove_count += 1;
                if vault.read_note(&moved_path).is_ok() {
                    index.upsert_note(vault, &moved_path)?;
                    stats.note_upsert_count += 1;
                    stats.note_move_count += 1;
                }
            }
        }
    }

    for path in note_removed_set {
        index.remove_note(&path);
        stats.note_remove_count += 1;
    }

    for (from, to) in note_moves_vec {
        index.remove_note(&from);
        stats.note_remove_count += 1;
        if vault.read_note(&to).is_ok() {
            index.upsert_note(vault, &to)?;
            stats.note_upsert_count += 1;
        }
        stats.note_move_count += 1;
    }

    for path in note_changed_set {
        if vault.read_note(&path).is_ok() {
            index.upsert_note(vault, &path)?;
            stats.note_upsert_count += 1;
        }
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_vault_root(name: &str) -> PathBuf {
        let mut root = std::env::temp_dir();
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        root.push(format!(
            "xnote_xtask_{name}_{}_{}",
            std::process::id(),
            now_nanos
        ));
        root
    }

    #[test]
    fn synthesize_watch_txn_replay_changes_uses_1k_plus_directory_batch() {
        let sample_notes = vec!["notes/a.md".to_string(), "notes/b.md".to_string()];
        let changes = synthesize_watch_txn_replay_changes(&sample_notes, 1_024);

        let folder_ops = changes
            .iter()
            .filter(|change| {
                matches!(
                    change,
                    VaultWatchChange::FolderCreated { .. }
                        | VaultWatchChange::FolderMoved { .. }
                        | VaultWatchChange::FolderRemoved { .. }
                )
            })
            .count();
        assert!(
            folder_ops >= 1_024,
            "expected >= 1024 folder ops, got {folder_ops}"
        );
        assert!(
            changes.len() > folder_ops,
            "should include mixed note changes"
        );
    }

    #[test]
    fn apply_watch_changes_benchmark_skips_rebuild_for_irrelevant_large_folder_batch() {
        let root = temp_vault_root("watch_replay_skip_rebuild_irrelevant");
        fs::create_dir_all(root.join("notes")).expect("create notes");
        fs::write(root.join("notes").join("alpha.md"), "# Alpha\n").expect("write alpha");
        fs::write(root.join("notes").join("beta.md"), "# Beta\n").expect("write beta");

        let vault = Vault::open(&root).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let mut index =
            KnowledgeIndex::build_from_entries(&vault, &entries).expect("build knowledge index");
        let base_note_count = index.note_count();

        let changes = synthesize_watch_txn_replay_changes(
            &entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>(),
            1_024,
        );
        let stats = apply_watch_changes_benchmark(&mut index, &vault, changes)
            .expect("apply watch changes");

        assert_eq!(stats.rebuild_count, 0);
        assert_eq!(index.note_count(), base_note_count);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_watch_changes_benchmark_note_only_stays_incremental() {
        let root = temp_vault_root("watch_replay_note_only");
        fs::create_dir_all(root.join("notes")).expect("create notes");
        fs::write(root.join("notes").join("draft.md"), "# Draft\nold\n").expect("write draft");

        let vault = Vault::open(&root).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let mut index =
            KnowledgeIndex::build_from_entries(&vault, &entries).expect("build knowledge index");

        vault
            .write_note("notes/draft.md", "# Draft\nnew\n")
            .expect("rewrite draft");
        let stats = apply_watch_changes_benchmark(
            &mut index,
            &vault,
            vec![VaultWatchChange::NoteChanged {
                path: "notes/draft.md".to_string(),
            }],
        )
        .expect("apply note change");

        assert_eq!(stats.rebuild_count, 0);
        assert_eq!(stats.note_upsert_count, 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_watch_changes_benchmark_applies_folder_move_incrementally() {
        let root = temp_vault_root("watch_replay_folder_move_incremental");
        fs::create_dir_all(root.join("notes").join("a")).expect("create notes/a");
        fs::write(root.join("notes").join("a").join("moved.md"), "# Move\n").expect("write moved");

        let vault = Vault::open(&root).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let mut index =
            KnowledgeIndex::build_from_entries(&vault, &entries).expect("build knowledge index");

        fs::create_dir_all(root.join("notes").join("b")).expect("create notes/b");
        fs::rename(
            root.join("notes").join("a").join("moved.md"),
            root.join("notes").join("b").join("moved.md"),
        )
        .expect("move note");
        let _ = fs::remove_dir_all(root.join("notes").join("a"));

        let stats = apply_watch_changes_benchmark(
            &mut index,
            &vault,
            vec![VaultWatchChange::FolderMoved {
                from: "notes/a".to_string(),
                to: "notes/b".to_string(),
            }],
        )
        .expect("apply folder move");

        assert_eq!(stats.rebuild_count, 0);
        assert!(index.note_summary("notes/a/moved.md").is_none());
        assert!(index.note_summary("notes/b/moved.md").is_some());

        let _ = fs::remove_dir_all(&root);
    }
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
