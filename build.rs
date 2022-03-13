use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use walkdir::WalkDir;
use xml::writer::{EmitterConfig, XmlEvent};

fn main() {
    compile_gresource();

    println!(
        "cargo:rustc-env=CARGO_OUT_DIR={}",
        env::var("OUT_DIR").unwrap()
    );

    println!("cargo:rerun-if-changed=data");
}

fn create_gresource_xml(
    prefix: &str,
    resource_dir: &str,
    resource_xml_path: &str,
) -> xml::writer::Result<()> {
    let mut file = File::create(resource_xml_path).unwrap();
    let mut writer = EmitterConfig::new()
        .perform_indent(true)
        .create_writer(&mut file);

    writer.write(XmlEvent::start_element("gresources"))?;
    writer.write(XmlEvent::start_element("gresource").attr("prefix", prefix))?;

    let skipped_file_names = vec!["meson.build", "resources.gresource.xml"];
    for entry in WalkDir::new(resource_dir)
        .sort_by_file_name()
        .into_iter()
        .flatten()
    {
        if entry.path().is_file() {
            let path = entry.path().strip_prefix(resource_dir);
            let filename = entry.file_name().to_str();

            if let Ok(Some(path)) = path.map(|p| p.to_str()) {
                if let Some(filename) = filename {
                    if !skipped_file_names.contains(&filename) {
                        writer.write(XmlEvent::start_element("file"))?;
                        writer.write(XmlEvent::characters(path))?;
                        writer.write(XmlEvent::end_element())?;
                    }
                }
            }
        }
    }

    writer.write(XmlEvent::end_element())?;
    writer.write(XmlEvent::end_element())?;

    Ok(())
}

fn files_identical(a: &Path, b: &Path) -> std::io::Result<bool> {
    let mut buf_a = Vec::new();
    let mut buf_b = Vec::new();

    if let Ok(mut file_a) = File::open(a) {
        file_a.read_to_end(&mut buf_a)?;
    } else {
        return Ok(false);
    }

    if let Ok(mut file_b) = File::open(b) {
        file_b.read_to_end(&mut buf_b)?;
    } else {
        return Ok(false);
    }

    Ok(buf_a.eq(&buf_b))
}

fn compile_gresource() {
    let out_dir_str = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_str);

    let target_file = out_dir.join("resources.gresource");

    let resource_file = out_dir.join("resources.gresource.xml");
    let resource_dir = Path::new("data/resources");

    create_gresource_xml(
        "/net/felinira/warp",
        resource_dir.to_str().unwrap(),
        resource_file.to_str().unwrap(),
    )
    .unwrap();

    let in_tree_resources = resource_dir.join("resources.gresource.xml");
    if !files_identical(&resource_file, &in_tree_resources).unwrap() {
        std::fs::copy(&resource_file, &in_tree_resources).unwrap();
    }

    let target_arg = format!("--target={}", target_file.to_str().unwrap());
    let source_arg = format!("--sourcedir={}", resource_dir.to_str().unwrap());

    let file_arg = resource_file.to_str().unwrap();

    let process = Command::new("glib-compile-resources")
        .args(&[&target_arg, &source_arg, file_arg])
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Error running glib-compile-resources");

    match process.wait_with_output() {
        Ok(output) => {
            if !output.status.success() {
                println!("cargo:warning=glib-compile-resources returned error. See output below:");
                println!(
                    "cargo:warning={}",
                    std::string::String::from_utf8(output.stderr).expect("UTF-8 error")
                );
                panic!();
            }
        }
        Err(err) => panic!("Error running glib-compile-resources: {}", err),
    };
}
