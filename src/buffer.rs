use std::{array, collections::HashMap, rc::Rc, cell::RefCell};
use std::hash::{BuildHasher, Hasher};
use crate::hashing::{BuildBufferHasher, BufferHasher};
use crate::processes::sleep;
use crate::disk::DiskDriver;

// used in the BufferQueue
const MAX_BUFFERS_PER_QUEUE : u64 = 10;



/// Possible states of buffers. More to be added maybe
#[derive(Debug,Clone)]
enum BufferStatus{
    Empty,
    Locked,
    Unlocked,
    DelayedWriteToDisk
}

/// a buffer header has some metadata for the buffer
/// and points to the data held in the buffer.
/// In this code I use "Buffer" and "Buffer Header"
/// interchangeably but I mean buffer header
#[derive(Debug,Clone)]
struct BufferHeader {
    device_num : u64,
    block_num : u64,
    status : BufferStatus,
    data : Option<Box<String>>
}

impl BufferHeader{
    ///returns the device number that the data the buffer is holding is from
    pub fn get_device_num(&self) -> u64 {
        self.device_num
    }

    /// returns the block number that the data the buffer is holding is from
    pub fn get_block_num(&self) -> u64 {
        self.block_num
    }


    /// returns : (device_num, block_num) 
    pub fn get_nums(&self) -> (u64, u64){
        (self.get_device_num(), self.get_block_num())
    } 

    
}


impl Default for BufferHeader {
    ///by default a buffer is empty and points to no data from no block or device
    fn default() -> Self {
        BufferHeader{ 
            device_num: 0,
            block_num: 0,
            status : BufferStatus::Empty,
            data : None
        }
    }
}

/// Just a specialized wrapper around a vector of
/// reference counting pointers pointing to BufferHeaders
///
// TODO: probably due for a refactoring to be more generic
struct FreeList{
    my_list : Vec<Rc<RefCell<BufferHeader>>>
}

impl FreeList {
    /// add a buffer to the end of the free list (most recently used)
    pub fn push(&self, buffer : Rc<RefCell<BufferHeader>>){
        self.my_list.push(buffer);
    }

    /// pop a buffer from the front of the free list (least recently used)
    pub fn pop(&self) -> Option<Rc<RefCell<BufferHeader>>> {
        self.my_list.pop()
    }

    /// remove the buffer with the given (block_num, device_num) from the free
    /// list. These should in theory always exist on the list
    pub fn remove(&self, buffer_nums : (u64, u64)){
        for (i, buf) in self.my_list.iter().enumerate(){
            if buf.get_nums() == buffer_nums{
                self.my_list.remove(i);
                return
            }
        }
    }

    /// is the free list empty?
    pub fn is_empty(&self) -> bool {
        self.my_list.len() == 0
    }

}


/// The BufferHashQueue is series of queues which are indexed
/// via a hash function. From my understanding the idea is to
/// maximize lookup speed 
struct BufferHashQueue {
    number_of_queues : u64,
    my_queues : Vec<Vec<Rc<RefCell<BufferHeader>>>>,
    my_hash_builder : BuildBufferHasher,
    my_hasher : BufferHasher
}

impl BufferHashQueue{

    /// create a new BufferHashQueue with number_of_queues queues.
    /// at this point in time there is no way to alter the number
    /// of queues after instantiation
    pub fn new(number_of_queues : u64) -> BufferHashQueue{
        let my_hash_builder = BuildBufferHasher{ positions : number_of_queues};

        BufferHashQueue{
            number_of_queues,
            my_queues : Vec::with_capacity(number_of_queues as usize),
            my_hash_builder,
            my_hasher : my_hash_builder.build_hasher()
        }
    }

    /// retrieve the buffer with the given device_num and block_num
    /// from the queues. If the buffer is not found: Option::None
    pub fn get_buffer(&self, device_num : u64 , block_num : u64) -> Option<Rc<RefCell<BufferHeader>>>{
        let index = self.hash_nums(device_num, block_num);
        let queue_to_search = self.my_queues[index as usize];
        for header in queue_to_search{
            let borrowed_header = *header.borrow();
            if borrowed_header.block_num == device_num && borrowed_header.block_num == block_num{
                return Some(header)
            }
        }
        None
    }

    /// append a buffer to it's proper hash queue
    pub fn add_buffer(&self, buffer_to_add: Rc<RefCell<BufferHeader>>){
        let index = self.hash_header(&buffer_to_add);
        self.my_queues[index as usize].push(buffer_to_add)
    }

    // hash a buffer header using hash_nums(device_num, block_num)
    fn hash_header(&self, buffer_to_hash: &BufferHeader) -> u64{
        self.hash_nums(buffer_to_hash.device_num, buffer_to_hash.block_num)
    }

    // hash a device_num and block_num
    fn hash_nums(&self, device_num : u64, block_num : u64) -> u64{
        let sum = block_num + device_num;
        self.my_hasher.write(&sum.to_be_bytes());
        self.my_hasher.finish()
    }
}


/// this is the system that manages buffers between the
/// free list, HashQueues and the drive
struct BufferSystem {
    free_list : FreeList,
    hash_queue : BufferHashQueue,
    disk_driver : DiskDriver
}

impl BufferSystem {
    
    /// create a new BufferSystem with 
    /// number_of_queues queues and number_of_buffers buffers
    pub fn new(number_of_queues : u64, number_of_buffers : u64) -> BufferSystem{
        
        // setting up the internal components of the system
        let free_list = vec![Rc::new(BufferHeader::default()); number_of_buffers as usize];
        let free_list = FreeList{ my_list : free_list };
        let hash_queue = BufferHashQueue::new(number_of_queues);
        
        BufferSystem{
            free_list,
            hash_queue,
            disk_driver: DiskDriver{data: Box::new("".to_owned())}
        }
    }

    /// get the block of memory specified by the block_num and device_num either
    /// from a buffer or from the disk.
    /// this algorithm is out of the book "The Design of the Unix Operating System"
    /// and is refered to as "getblk" in that book
    fn get_block(&self, device_num : u64, block_num : u64) -> Rc<RefCell<BufferHeader>>{
        loop{
            let retrieved = self.hash_queue.get_buffer(device_num, block_num);
            match retrieved{
                Some(mut buffer) => {
                    let borrowed_buffer = *buffer.borrow();
                    if let BufferStatus::Locked = borrowed_buffer.status {
                            sleep("Buffer becomes free");
                            continue;
                        }
                        borrowed_buffer.status = BufferStatus::Locked;
                    self.free_list.remove(borrowed_buffer.get_nums());
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

