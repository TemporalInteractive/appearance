use std::io::{Read, Write};
use std::net::TcpStream;

use anyhow::Result;
use std::thread;

use crate::host::{HostMessage, HostMessageType};

#[derive(bytemuck::NoUninit, bytemuck::AnyBitPattern, Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct NodeScissor {
    pub scissor_x: [u32; 2],
    pub scissor_y: [u32; 2],
}

impl NodeScissor {
    pub fn width(&self) -> u32 {
        self.scissor_x[1] - self.scissor_x[0]
    }

    pub fn height(&self) -> u32 {
        self.scissor_y[1] - self.scissor_y[0]
    }
}

pub trait NodeRenderer: Send {
    // TODO: world manipulation

    fn render(&mut self, width: u32, height: u32, scissor: NodeScissor) -> &[u8];
}

pub struct Node<T: NodeRenderer> {
    host_addr: String,
    tcp_stream: Option<TcpStream>,

    renderer: T,
}

impl<T: NodeRenderer + 'static> Node<T> {
    pub fn new(renderer: T, host_addr: &str) -> Result<Self> {
        Ok(Self {
            host_addr: host_addr.to_owned(),
            tcp_stream: None,
            renderer,
        })
    }

    fn handle_message(&mut self, host_message: HostMessage) -> Result<()> {
        match host_message.ty() {
            HostMessageType::StartRender => {
                if let Some(tcp_stream) = &mut self.tcp_stream {
                    let pixels = self.renderer.render(
                        host_message.width,
                        host_message.height,
                        host_message.scissor,
                    );

                    tcp_stream.write_all(pixels)?;
                }
            }
            HostMessageType::Ping => {}
        }

        Ok(())
    }

    fn disconnect(&mut self) {
        log::info!("Disconnected");
        self.tcp_stream = None;
    }

    pub fn run(mut self) {
        let mut buf = vec![0u8; std::mem::size_of::<HostMessage>()];

        loop {
            // Try to connect to host if not connected yet
            if self.tcp_stream.is_none() {
                if let Ok(tcp_stream) = TcpStream::connect(&self.host_addr) {
                    log::info!("Connected");
                    self.tcp_stream = Some(tcp_stream);
                }
            }

            if let Some(tcp_stream) = &mut self.tcp_stream {
                if let Ok(_len) = tcp_stream.read(buf.as_mut()) {
                    let host_message = *bytemuck::from_bytes::<HostMessage>(buf.as_ref());

                    log::info!("Received message: {:?}", host_message);
                    if self.handle_message(host_message).is_err() {
                        self.disconnect();
                    }
                } else {
                    self.disconnect();
                }
            }

            thread::yield_now();
        }
    }
}
