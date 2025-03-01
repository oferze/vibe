use anyhow::Result;
use clap::{ArgAction, Parser};
use env_logger;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};

use std::{fmt::Write, path::PathBuf, sync::Mutex};
use vibe;

static PROGRESS_INSTANCE: once_cell::sync::Lazy<Mutex<Option<ProgressBar>>> = once_cell::sync::Lazy::new(|| Mutex::new(None));
static PROGRESS_INSTANCE_ASYNC: once_cell::sync::Lazy<tokio::sync::Mutex<Option<ProgressBar>>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(None));

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to file
    #[arg(short, long)]
    pub path: String,

    /// Path to model
    #[arg(short, long)]
    pub model: Option<String>,

    /// Language spoken in the audio. Attempts to auto-detect by default.
    #[clap(short, long)]
    pub lang: Option<vibe::language::Language>,

    /// Verbose output
    #[arg(long, action=ArgAction::SetTrue, default_value_t = false)]
    pub verbose: bool,

    /// N threads for model
    #[arg(long)]
    pub n_threads: Option<i32>,
}

async fn on_download_progress(current: u64, total: u64) {
    if let Some(pb) = PROGRESS_INSTANCE_ASYNC.lock().await.as_ref() {
        pb.set_position(current / total * 100 as u64);
    }
}

fn on_transcribe_progress(progress: i32) {
    if let Some(pb) = PROGRESS_INSTANCE.lock().unwrap().as_ref() {
        pb.set_position(progress as u64);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
        })
        .progress_chars("#>-"),
    );
    *PROGRESS_INSTANCE.lock().unwrap() = Some(pb.clone());
    let args = Args::parse();
    let mut downloader = vibe::downloader::Downloader::new();
    if !vibe::config::get_model_path()?.exists() {
        downloader
            .download(
                vibe::config::URL,
                vibe::config::get_model_path()?,
                Some(""),
                on_download_progress,
            )
            .await?;
        pb.finish_with_message("Download complete");
    }
    pb.reset_eta();

    let args = vibe::config::ModelArgs {
        lang: args.lang.and_then(|a| Some(a.as_str().to_string())),
        model: vibe::config::get_model_path()?,
        path: PathBuf::from(args.path),
        n_threads: args.n_threads,
        verbose: args.verbose,
    };
    let transcript = vibe::model::transcribe(&args, Some(on_transcribe_progress))?;
    println!("{}", transcript.as_srt());
    Ok(())
}
