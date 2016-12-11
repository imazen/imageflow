use ::std;
use ::preludes::from_std::*;


pub fn read_file_bytes<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<u8>>{
    let mut f = OpenOptions::new().read(true).create(false).open(path)?;
    let mut data = Vec::new();
    f.read_to_end(&mut data)?;
    Ok((data))
}