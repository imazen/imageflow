use std;
use crate::preludes::from_std::*;
use zip;

pub fn read_file_bytes<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<u8>>{
    let mut f = OpenOptions::new().read(true).create(false).open(path)?;
    let mut data = Vec::new();
    f.read_to_end(&mut data)?;
    Ok(data)
}

pub fn zip_directory_nonrecursive<P: AsRef<Path>>(dir: P, archive_name: P) -> zip::result::ZipResult<()> {
    let mut zip = zip::ZipWriter::new(File::create(archive_name.as_ref()).unwrap());

    let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.add_directory(archive_name.as_ref().file_stem().unwrap().to_str().unwrap().to_owned(), options)?;
    let entries = std::fs::read_dir(dir.as_ref()).unwrap();

    for entry_maybe in entries {
        if let Ok(entry) = entry_maybe {
            let file_name = entry.file_name().into_string().unwrap();
            if file_name.starts_with('.') {
                //skipping
            } else if entry.path().is_file() {
                let mut file = File::open(entry.path()).unwrap();
                let mut contents = Vec::new();
                file.read_to_end(&mut contents).unwrap();

                let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

                zip.start_file(file_name, options)?;
                zip.write_all(&contents)?;
            }
        }
        //println!("Name: {}", path.unwrap().path().display())
    }

    zip.finish()?;
    Ok(())
}
