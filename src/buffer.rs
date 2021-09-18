use std::{array, collections::HashMap, rc::Rc};
use std::hash::{BuildHasher, Hasher};
use crate::hashing::{BuildBufferHasher, BufferHasher};
use crate::processes::sleep;
use crate::disk::DiskDriver;


const MAX_BUFFERS_PER_QUEUE : u64 = 10;

#[derive(Debug,Clone)]
enum BufferStatus{
    Empty,
    Locked,
    Unlocked,
    DelayedWriteToDisk
}

#[derive(Debug,Clone)]
struct BufferHeader {
    device_num : u64,
    block_num : u64,
    status : BufferStatus,
    data : Option<Box<String>>
}

impl BufferHeader{
    pub fn get_device_num(&self) -> u64 {
        self.device_num
    }

    pub fn get_block_num(&self) -> u64 {
        self.block_num
    }


    // returns : (device_num, block_num) 
    pub fn get_nums(&self) -> (u64, u64){
        (self.get_device_num(), self.get_block_num())
    } 

    
}

impl Default for BufferHeader {
    fn default() -> Self {
        BufferHeader{
            device_num: 0,
            block_num: 0,
            status : BufferStatus::Empty,
            data : None
        }
    }
}

struct FreeList{
    my_list : Vec<Rc<BufferHeader>>
}

impl FreeList {
    pub fn push(&self, buffer : Rc<BufferHeader>){
        self.my_list.push(buffer);
    }

    pub fn pop(&self) -> Option<Rc<BufferHeader>> {
        self.my_list.pop()
    }

    pub fn remove(&self, buffer_nums : (u64, u64)){
        for (i, buf) in self.my_list.iter().enumerate(){
            if buf.get_nums() == buffer_nums{
                self.my_list.remove(i);
                return
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.my_list.len() == 0
    }

}



#[derive(Debug,Clone)]
struct HashQueueHeader {
    file_system_number: usize,
    block_number : usize
}



struct BufferHashQueue {
    number_of_queues : u64,
    my_queues : Vec<Vec<Rc<BufferHeader>>>,
    my_hash_builder : BuildBufferHasher,
    my_hasher : BufferHasher
}

impl BufferHashQueue{
    pub fn new(number_of_queues : u64) -> BufferHashQueue{
        let my_hash_builder = BuildBufferHasher{ positions : number_of_queues};

        BufferHashQueue{
            number_of_queues,
            my_queues : Vec::with_capacity(number_of_queues as usize),
            my_hash_builder,
            my_hasher : my_hash_builder.build_hasher()
        }
    }

    pub fn get_buffer(&self, device_num : u64 , block_num : u64) -> Option<Rc<BufferHeader>>{
        let index = self.hash_nums(device_num, block_num);
        let queue_to_search = self.my_queues[index as usize];
        for header in queue_to_search{
            if header.device_num == device_num && header.block_num == block_num{
                return Some(header)
            }
        }
        None
    }

    pub fn add_buffer(&self, buffer_to_add: Rc<BufferHeader>){
        let index = self.hash_header(&buffer_to_add);
        self.my_queues[index as usize].push(buffer_to_add)
    }

    fn hash_header(&self, buffer_to_hash: &BufferHeader) -> u64{
        self.hash_nums(buffer_to_hash.device_num, buffer_to_hash.block_num)
    }

    fn hash_nums(&self, device_num : u64, block_num : u64) -> u64{
        let sum = block_num + device_num;
        self.my_hasher.write(&sum.to_be_bytes());
        self.my_hasher.finish()
    }


}


struct BufferSystem {
    free_list : FreeList,
    hash_queue : BufferHashQueue,
    disk_driver : DiskDriver
}

impl BufferSystem {
    
    pub fn new(number_of_queues : u64, number_of_buffers : u64) -> BufferSystem{
        let free_list = vec![Rc::new(BufferHeader::default()); number_of_buffers as usize];
        let free_list = FreeList{ my_list : free_list };
        let hash_queue = BufferHashQueue::new(number_of_queues);
        
        BufferSystem{
            free_list,
            hash_queue,
            disk_driver: DiskDriver{data: Box::new("".to_owned())}
        }
    }

    fn get_block(&self, device_num : u64, block_num : u64) -> Rc<BufferHeader>{
        loop{
            let retrieved = self.hash_queue.get_buffer(device_num, block_num)
            match retrieved{
                Some(buffer) => {
                    if let BufferStatus::Locked = buffer.status {
                            sleep("Buffer becomes free");
                            continue;
                        }
                    buffer.status = BufferStatus::Locked;
                    self.free_list.remove(buffer.get_nums());
                    return buffer
                },
                None => {
                    if self.free_list.is_empty(){
                        sleep("Any buffer becomes free");
                        continue;
                    }
                    self.free_list.remove((device_num, block_num));
                    
                    
                }
            }
        }
    }
}

