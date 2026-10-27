//! One large-stack pipeline worker and one replaceable waiting request.
use super::pack_post::{OwnedWorld, PackPost};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, TryRecvError},
};

pub(super) const STACK: usize = 16 * 1024 * 1024;
pub(super) struct Request {
    pub generation: u64,
    pub dimension: i32,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub post: horizon_shaders::Post,
    pub screen: wgpu::TextureFormat,
    pub world: OwnedWorld,
}
pub(super) struct Finished {
    pub generation: u64,
    pub dimension: i32,
    pub post: Option<PackPost>,
    pub programs: Vec<String>,
    pub notes: Vec<String>,
}
struct Running {
    generation: u64,
    dimension: i32,
    result: mpsc::Receiver<Finished>,
    cancel: Arc<AtomicBool>,
}
#[derive(Default)]
pub(super) struct Compiler {
    running: Option<Running>,
    queued: Option<Request>,
    failed: Option<Finished>,
}
fn failed(generation: u64, dimension: i32, message: String) -> Finished {
    Finished {
        generation,
        dimension,
        post: None,
        programs: Vec::new(),
        notes: vec![message],
    }
}
impl Compiler {
    pub fn submit(&mut self, request: Request) {
        if let Some(running) = &self.running {
            running.cancel.store(true, Ordering::Relaxed);
        }
        self.queued = Some(request);
        self.start();
    }
    pub fn cancel_queued(&mut self) {
        self.queued = None;
        self.failed = None;
        if let Some(running) = &self.running {
            running.cancel.store(true, Ordering::Relaxed);
        }
    }
    fn start(&mut self) {
        if self.running.is_some() {
            return;
        }
        let Some(request) = self.queued.take() else {
            return;
        };
        let (generation, dimension) = (request.generation, request.dimension);
        let (send, result) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        match std::thread::Builder::new()
            .name("Shader pipelines".into())
            .stack_size(STACK)
            .spawn(move || {
                // wgpu's std feature makes error scopes thread-local, and its
                // handles are Send/Sync. The render thread keeps its normal frame.
                let post = request.world.build(
                    &request.device,
                    &request.queue,
                    request.post,
                    request.screen,
                    &stop,
                );
                let mut notes = post.notes.clone();
                let programs: Vec<String> = if post.complete {
                    post.programs().into_iter().map(str::to_owned).collect()
                } else {
                    Vec::new()
                };
                let post = if post.complete && !programs.is_empty() && !stop.load(Ordering::Relaxed)
                {
                    Some(post)
                } else {
                    notes.push(
                        "Shader pipeline could not load completely. Using normal rendering.".into(),
                    );
                    None
                };
                let _ = send.send(Finished {
                    generation,
                    dimension,
                    post,
                    programs,
                    notes,
                });
            }) {
            Ok(_detached) => {
                self.running = Some(Running {
                    generation,
                    dimension,
                    result,
                    cancel,
                })
            }
            Err(error) => {
                self.failed = Some(failed(
                    generation,
                    dimension,
                    format!("Could not start shader worker: {error}. Using normal rendering."),
                ))
            }
        }
    }
    pub fn poll(&mut self) -> Option<Finished> {
        if let Some(result) = self.failed.take() {
            return Some(result);
        }
        let result = match self.running.as_ref() {
            Some(run) => match run.result.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => Some(failed(
                    run.generation,
                    run.dimension,
                    "Shader worker failed. Using normal rendering.".into(),
                )),
            },
            None => None,
        };
        if result.is_some() {
            self.running = None;
        }
        self.start();
        result.or_else(|| self.failed.take())
    }
}
impl Drop for Compiler {
    fn drop(&mut self) {
        self.cancel_queued();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_results_are_polled_and_failures_settle_once() {
        let (send, result) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let mut compiler = Compiler {
            running: Some(Running {
                generation: 4,
                dimension: -1,
                result,
                cancel: stop.clone(),
            }),
            queued: None,
            failed: None,
        };
        assert!(compiler.poll().is_none());
        compiler.cancel_queued();
        assert!(stop.load(Ordering::Relaxed));
        send.send(failed(4, -1, "test failure".into()))
            .unwrap_or_else(|_| panic!("receiver dropped"));
        let done = compiler.poll().unwrap();
        assert_eq!((done.generation, done.dimension), (4, -1));
        assert!(done.post.is_none());
        assert!(compiler.poll().is_none());
        assert!(compiler.running.is_none());
    }
    #[test]
    fn a_disconnected_worker_does_not_respawn_each_poll() {
        let (send, result) = mpsc::channel();
        drop(send);
        let mut compiler = Compiler {
            running: Some(Running {
                generation: 1,
                dimension: 0,
                result,
                cancel: Arc::new(AtomicBool::new(false)),
            }),
            queued: None,
            failed: None,
        };
        assert!(compiler.poll().unwrap().notes[0].contains("worker failed"));
        assert!(compiler.poll().is_none());
    }
}
