extern crate clap;

use clap::{App, Arg};
use imgref::ImgRef;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::str::from_utf8;

fn make_cache() {
    let cache_path = Path::new("cache");

    if cache_path.exists() {
        fs::remove_dir_all(cache_path).expect("failed to clear cache directory");
    }

    fs::create_dir(cache_path).expect("failed to create cache directory");
    fs::create_dir(&cache_path.join("hi_res")).expect("failed to create hi_res directory");
    fs::create_dir(&cache_path.join("lo_res")).expect("failed to create lo_res directory");
}

fn split_video(path: &Path, downscale: f32, verbosity: u64) {
    let path_str = path
        .to_str()
        .expect("failed to convert path to str when splitting video");

    match verbosity {
        0 => {}
        1 | _ => println!("splitting high res"),
    }

    let hi_res_out = Command::new("ffmpeg.exe")
        .arg("-i")
        .arg(path_str)
        .arg("cache/hi_res/%08d.png")
        .output()
        .expect("failed to execute ffmpeg");

    match verbosity {
        0 => {}
        1 | _ => {
            // let stdout = from_utf8(&hi_res_out.stdout).unwrap();
            let stderr = from_utf8(&hi_res_out.stderr).unwrap();

            println!("{}", stderr);
            println!("splitting low res");
        }
    }

    let lo_res_out = Command::new("ffmpeg.exe")
        .arg("-i")
        .arg(path_str)
        .arg("-vf")
        .arg(format!("scale=iw*{ds:.2}:ih*{ds:.2}", ds = downscale))
        .arg("cache/lo_res/%08d.png")
        .output()
        .expect("failed to execute ffmpeg");

    match verbosity {
        0 => {}
        1 | _ => {
            // let stdout = from_utf8(&lo_res_out.stdout).unwrap();
            let stderr = from_utf8(&lo_res_out.stderr).unwrap();

            println!("{}", stderr);
        }
    }
}

// todo: add option for alpha
fn open_to_dssim(dis: &dssim_core::Dssim, path: &Path) -> dssim_core::DssimImage<f32> {
    let img_rgb = image::open(path)
        .expect("failed to open image when trying to convert to dssim")
        .to_rgb();
    let img_rgb_norm = img_rgb
        .pixels()
        .map(|pix| Into::<rgb::RGB<f32>>::into(rgb::RGB::from(pix.0)) / 255.)
        .collect::<Vec<_>>();

    let img_ref = ImgRef::new(
        &img_rgb_norm,
        img_rgb.width() as usize,
        img_rgb.height() as usize,
    );

    dis.create_image(&img_ref)
        .expect("failed to create image with dssim")
}

fn image_seq_to_dssim_vec(
    dis: &dssim_core::Dssim,
    dir: &Path,
    verbosity: u64,
) -> Vec<dssim_core::DssimImage<f32>> {
    let mut dssim_frames: Vec<dssim_core::DssimImage<f32>> = vec![];
    let results_vec: Vec<Result<std::fs::DirEntry, std::io::Error>> = fs::read_dir(dir)
        .expect(&format!(
            "failed to read {} dir",
            &dir.file_name()
                .expect("failed to get directory name")
                .to_str()
                .expect("failed to convert directory name to str")
        ))
        .collect();
    let results_vec_len = results_vec.len();

    for result in results_vec.iter().enumerate() {
        if let Ok(dir_entry) = result.1 {
            match verbosity {
                0 => {}
                1 | _ => println!(
                    "opening frame {} of {} to dssim image ({:.2}%)",
                    result.0 + 1,
                    results_vec_len,
                    (result.0 + 1) as f32 / results_vec_len as f32 * 100 as f32
                ),
            }

            dssim_frames.push(open_to_dssim(dis, &dir_entry.path()));
        }
    }

    dssim_frames
}

fn remove_duplicate_frames(threshold: f64, verbosity: u64) {
    match verbosity {
        0 => {}
        1 | _ => println!("remove_duplicate_frames"),
    }

    let dis = dssim_core::new();
    let dssim_frames = &image_seq_to_dssim_vec(&dis, Path::new("cache/lo_res"), verbosity);
    let mut dupes = std::collections::HashSet::new();
    let dssim_frames_len = dssim_frames.len();

    for i in 0..dssim_frames_len {
        if dupes.contains(&i) {
            continue;
        }

        let prog = (i + 1) as f32 / dssim_frames_len as f32 * 100 as f32;

        for j in i + 1..dssim_frames.len() {
            if dupes.contains(&j) {
                continue;
            }

            let dis_comp = dis.compare(&dssim_frames[i], &dssim_frames[j]).0;
            let is_dupe = dis_comp < threshold; // i changed this to < as we want dssim UNDER a threshold

            if is_dupe {
                match verbosity {
                    0 => {}
                    1 | _ => println!(
                        "found duplicate!\r\n{} < {} [{}] [{}] ({:.2}%) <{}>",
                        dis_comp,
                        threshold,
                        i + 1,
                        j + 1,
                        prog,
                        j - i
                    ),
                }

                dupes.insert(j);

                fs::remove_file(Path::new(&format!(
                    "cache/hi_res/{index:>0width$}.png",
                    index = j + 1,
                    width = 8
                )))
                .expect("filed to remove duplicate image");
            }
        }
    }
}

fn renumber_image_seq(verbosity: u64) {
    match verbosity {
        0 => {}
        1 | _ => println!("renumber_image_seq"),
    }

    let results = fs::read_dir("cache/hi_res").expect("failed to read directory when renumbering");

    for result in results.enumerate() {
        if let Ok(dir_entry) = result.1 {
            fs::rename(
                Path::new(&dir_entry.path()),
                format!(
                    "cache/hi_res/{index:>0width$}.png",
                    index = result.0 + 1,
                    width = 8
                ),
            )
            .expect("failed to rename file when renumbering");
        }
    }
}

fn merge_image_seq(path: &Path, verbosity: u64) {
    match verbosity {
        0 => {}
        1 | _ => println!("merge_image_seq"),
    }

    let path_str = path
        .to_str()
        .expect("failed to convert path to string when merging");

    Command::new("ffmpeg.exe")
        .arg("-i")
        .arg("cache/hi_res/%08d.png")
        .arg("-y")
        .arg(path_str)
        .output()
        .expect("failed to merge image sequence");
}

fn main() {
    let matches = App::new("Drop Dupe")
        .version("1.0")
        .author("QuantumCoded <bfields32@student.cccs.edu>")
        .about("A tool for detecting and removing duplicate frames from different kinds of video")
        .arg(
            Arg::with_name("threshold")
                .short("t")
                .long("threshold")
                .value_name("THRESHOLD")
                .help("The dissimilarity threshold (smaller forces more similar duplicates)"),
        )
        .arg(
            Arg::with_name("downscale")
                .short("s")
                .long("scale")
                .value_name("DOWNSCALE")
                .help("The amount to scale the video by before checking (smaller = faster)"),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .multiple(true)
                .help("Sets verbosity level"),
        )
        .arg(
            Arg::with_name("INPUT")
                .help("Path to a video file")
                .required(true)
                .index(1),
        )
        .get_matches();

    let video_name = matches.value_of("INPUT").unwrap();
    let video_path = Path::new(&video_name);
    let threshold = matches
        .value_of("threshold")
        .unwrap_or("0.0025")
        .parse::<f64>()
        .unwrap();
    let downscale = matches
        .value_of("downscale")
        .unwrap_or("0.125")
        .parse::<f32>()
        .unwrap();
    let verbosity = matches.occurrences_of("verbose");

    println!(
        "running with threshold={}, downscale={}, verbosity={}",
        threshold, downscale, verbosity
    );

    make_cache();
    split_video(&video_path, downscale, verbosity);
    remove_duplicate_frames(threshold, verbosity);
    renumber_image_seq(verbosity);
    merge_image_seq(Path::new("out.mp4"), verbosity);

    println!("finished");
}
