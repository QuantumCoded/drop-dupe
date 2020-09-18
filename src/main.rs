use imgref::ImgRef;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn split_video(path: &Path, downscale: f32) {
    println!("split_video");

    let path_str = path
        .to_str()
        .expect("failed to convert path to str when splitting video");

    Command::new("ffmpeg.exe")
        .arg("-i")
        .arg(path_str)
        .arg("-vf")
        .arg(format!("scale=iw*{ds:.2}:ih*{ds:.2}", ds = downscale))
        .arg("temp/%08d.png")
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

fn image_seq_to_dssim_vec(dis: &dssim_core::Dssim) -> Vec<dssim_core::DssimImage<f32>> {
    let mut dssim_frames: Vec<dssim_core::DssimImage<f32>> = vec![];
    let results_vec: Vec<Result<std::fs::DirEntry, std::io::Error>> = fs::read_dir("temp")
        .expect("failed to read temp dir")
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

fn find_duplicate_frames(threshold: f64) -> Vec<u32> {
    let dis = dssim_core::new();
    let dssim_frames = image_seq_to_dssim_vec(&dis);
    let mut dupes = std::collections::HashSet::new();
    let dssim_frames_len = dssim_frames.len();

    for i in 0..dssim_frames_len {
        if dupes.contains(&i) {
            continue;
        }

        println!(
            "compare {} of {} ({:.2}%)",
            i + 1,
            dssim_frames_len,
            (i + 1) as f32 / dssim_frames_len as f32 * 100 as f32
        );

        for j in i + 1..dssim_frames.len() {
            if dupes.contains(&j) {
                continue;
            }

            let dis_comp = dis.compare(&dssim_frames[i], &dssim_frames[j]).0;
            let is_dupe = dis_comp < threshold; // i changed this to <= as we want dssim UNDER a threshold

            println!(
                "{} < {} = {} [{}] [{}]",
                dis_comp,
                threshold,
                is_dupe,
                i + 1,
                j + 1
            );

            if is_dupe {
                dupes.insert(j);
            }
        }
    }

    dssim_frames
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| dupes.contains(idx))
        .map(|(idx, _)| idx as u32)
        .collect()
}

fn remove_duplicate_frames(threshold: f64) {
    println!("remove_duplicate_frames");

    let dupes = find_duplicate_frames(threshold);

    for idx in dupes {
        let file_name = format!("temp/{index:>0width$}.png", index = idx + 1, width = 8);
        let image = Path::new(&file_name);

        fs::remove_file(image).expect("failed to remove duplicate image");
    }
}

fn renumber_image_seq() {
    println!("renumber_image_seq");

    let results = fs::read_dir("temp").expect("failed to read temp directory when renumbering");

    for result in results.enumerate() {
        if let Ok(dir_entry) = result.1 {
            fs::rename(
                Path::new(&dir_entry.path()),
                format!("temp/{index:>0width$}.png", index = result.0 + 1, width = 8),
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
        .arg("temp/%08d.png")
        .arg("-y")
        .arg(path_str)
        .output()
        .expect("failed to merge image sequence");
}

fn main() {
    let video_name = env::args().nth(1).expect("expected video file");
    let video_path = Path::new(&video_name);
    let threshold = 0.01;
    let downscale = 0.15;

    // Remove the temp directory and its contents if it exists to prevent weird stuff
    if Path::new("temp").exists() {
        fs::remove_dir_all("temp").expect("failed to remove temp directory");
    }

    fs::create_dir(Path::new("temp")).expect("failed to create temp directory");

    split_video(&video_path, downscale);
    remove_duplicate_frames(threshold);
    renumber_image_seq();
    merge_image_seq(Path::new("out.mp4"));
}
