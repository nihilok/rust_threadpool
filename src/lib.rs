extern crate core;

use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);
        println!("Starting threadpool with {} workers", size);
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, f: F)
        where
            F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());
        println!("Gracefully slaughtering workers...");
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();
            match message {
                Ok(job) => {
                    job();
                }
                Err(_) => {
                    println!("Worker {} is dead", id);
                    break;
                }
            }
        });

        Worker {
            thread: Some(thread),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::fs;
    use std::fs::OpenOptions;
    use std::io::prelude::*;
    use std::fs::File;
    use std::io::{self, BufRead};
    use std::path::Path;

    const FILENAME: &str = "test.file";
    const THREADS: usize = 4;

    fn create_file() {
        let _ = fs::write(FILENAME, vec![]).unwrap();
    }

    fn add_to_file(val: u8) {
        println!("Writing {} to file", val);

        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(FILENAME)
            .unwrap();

        if let Err(e) = writeln!(file, "{}", val) {
            eprintln!("Couldn't write to file: {}", e);
        }
    }

    fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
        where P: AsRef<Path>, {
        let file = File::open(filename)?;
        Ok(io::BufReader::new(file).lines())
    }

    #[test]
    fn it_works() {
        create_file();

        let threadpool = ThreadPool::new(THREADS);

        for i in 1..=3 {
            threadpool.execute(move || {
                thread::sleep(Duration::from_secs(i as u64));
                add_to_file(i as u8);
            })
        }
        threadpool.execute(|| {
            add_to_file(0 as u8);
        });

        for (i, line) in read_lines(FILENAME).expect("Couldn't read file").enumerate() {
            if i < THREADS {
                let result = line.unwrap().trim().parse::<usize>().unwrap();
                assert_eq!(i, result)
            }
        }
    }
}
