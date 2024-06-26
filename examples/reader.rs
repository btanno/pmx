use std::fs::File;
use std::io::BufReader;

fn main() -> anyhow::Result<()> {
    let file = File::open("assets/Alicia/Alicia_solid.pmx")?;
    let reader = pmx::Reader::new(BufReader::new(file))?;
    println!("[name] {}", reader.name());
    println!("[name EN] {}", reader.name_en());
    println!("[comment]\n{}", reader.comment());
    println!("[comment EN]\n{}", reader.comment_en());
    println!("[textures : {}]", reader.textures().len());
    for texture in reader.textures() {
        println!("{}", texture.to_string_lossy());
    }
    println!("[materials : {}]", reader.materials().len());
    for material in reader.materials() {
        println!("{}", material.name);
    }
    Ok(())
}
