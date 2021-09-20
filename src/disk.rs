use std::borrow::BorrowMut;


///at the moment this is just a mock of a disk driver
pub struct DiskDriver{
    data: Box<String>
}

impl DiskDriver {
    pub fn write(&self, data : &str){
        self.data.push_str(data);
    }
}



pub fn write_to_disk(data: &str){
    print!("Writing \"{}\" to disk", data )
}