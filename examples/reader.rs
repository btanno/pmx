use std::fs::File;
use std::io::BufReader;

fn main() -> anyhow::Result<()> {
    let file = File::open("assets/Alicia/Alicia_solid.pmx")?;
    let reader = pmx::Reader::new(BufReader::new(file))?;
    println!("[name] {}", reader.name());
    println!("[name EN] {}", reader.name_en());
    println!("[comment]\n{}", reader.comment());
    println!("[comment EN]\n{}", reader.comment_en());
    println!("[vertices] len = {}", reader.vertices().len());
    println!("[faces] len = {}", reader.faces().len());
    println!("[textures] len = {}", reader.textures().len());
    for texture in reader.textures() {
        println!("{}", texture.to_string_lossy());
    }
    println!("[materials] len = {}", reader.materials().len());
    for material in reader.materials() {
        println!("{}", material.name);
    }
    println!("[bones] len = {}", reader.bones().len());
    for bone in reader.bones() {
        println!("{}", bone.name);
    }
    println!("[rigids] len = {}", reader.rigids().len());
    for rigid in reader.rigids() {
        println!("{}", rigid.name);
    }
    println!("[joints] len = {}", reader.joints().len());
    for joint in reader.joints() {
        println!("{}", joint.name);
    }
    Ok(())
}
