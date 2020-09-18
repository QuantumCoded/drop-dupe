use imgref::ImgRef;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn make_cache() {
    let cache_path = Path::new("cache");

    if cache_path.exists() {
        fs::remove_dir_all(cache_path).expect("failed to clear cache directory");
    }

    fs::create_dir(cache_path).expect("failed to create cache directory");
    fs::create_dir(&cache_path.join("hi_res")).expect("failed to create hi_res directory");
    fs::create_dir(&cache_path.join("lo_res")).expect("failed to create lo_res directory");
}

fn split_video(path: &Path, downscale: f32) {
    let path_str = path
        .to_str()
        .expect("failed to convert path to str when splitting video");

    println!("splitting high res");
    Command::new("ffmpeg.exe")
        .arg("-i")
        .arg(path_str)
        .arg("cache/hi_res/%08d.png")
        .output()
        .expect("failed to split video");

    println!("splitting low res");
    Command::new("ffmpeg.exe")
        .arg("-i")
        .arg(path_str)
        .arg("-vf")
        .arg(format!("scale=iw*{ds:.2}:ih*{ds:.2}", ds = downscale))
        .arg("cache/lo_res/%08d.png")
        .output()
        .expect("failed to split video");
}

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

fn image_seq_to_dssim_vec(dis: &dssim_core::Dssim, dir: &Path) -> Vec<dssim_core::DssimImage<f32>> {
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
            println!(
                "opening frame {} of {} to dssim image ({:.2}%)",
                result.0 + 1,
                results_vec_len,
                (result.0 + 1) as f32 / results_vec_len as f32 * 100 as f32
            );
            dssim_frames.push(open_to_dssim(dis, &dir_entry.path()));
        }
    }

    dssim_frames
}

fn remove_duplicate_frames(threshold: f64) {
    let dis = dssim_core::new();
    let dssim_frames = &image_seq_to_dssim_vec(&dis, Path::new("cache/lo_res"));
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

            println!(
                "{} < {} = {} [{}] [{}] ({:.2}%)",
                dis_comp,
                threshold,
                is_dupe,
                i + 1,
                j + 1,
                prog
            );

            if is_dupe {
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

fn renumber_image_seq() {
    println!("renumber_image_seq");

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

fn merge_image_seq(path: &Path) {
    println!("merge_image_seq");

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
    let video_name = env::args().nth(1).expect("expected video file");
    let video_path = Path::new(&video_name);
    let threshold = 0.005;
    let downscale = 0.25;

    make_cache();
    split_video(&video_path, downscale);
    remove_duplicate_frames(threshold);
    renumber_image_seq();
    merge_image_seq(Path::new("out.mp4"));
}
