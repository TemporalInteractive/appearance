use core::{net::SocketAddr, ops::FnMut, sync::atomic::Ordering};
use glam::UVec2;
use std::thread;

use anyhow::Result;
use appearance_world::visible_world_action::VisibleWorldActionType;
use unreliable::{Socket, SocketEvent};

use crate::host::{
    HostToNodeMessage, NodeToHostMessage, RenderPartialFinishedData, StartRenderData,
    NODE_BYTES_PER_PIXEL, NODE_PIXEL_FORMAT, RENDER_BLOCK_SIZE,
};

pub trait NodeRenderer {
    // TODO: world manipulation
    fn visible_world_action(&mut self, action: &VisibleWorldActionType);

    fn render<F: FnMut(&[u8])>(
        &mut self,
        resolution: UVec2,
        start_row: u32,
        end_row: u32,
        result_callback: F,
    );
}

pub struct Node<T: NodeRenderer> {
    socket: Socket,
    host_port: u16,
    renderer: T,
}

impl<T: NodeRenderer + 'static> Node<T> {
    pub fn new(renderer: T, host_addr: SocketAddr, receiving_port: u16) -> Result<Self> {
        let socket = Socket::new(Some(host_addr), receiving_port)?;

        Ok(Self {
            socket,
            host_port: host_addr.port(),
            renderer,
        })
    }

    fn start_render(&mut self, data: StartRenderData, addr: &SocketAddr) {
        log::info!("start render: {:?}", data);

        self.renderer.render(
            UVec2::new(data.width, data.height),
            data.row_start,
            data.row_end,
            |pixels| {
                let num_blocks_x = data.width / RENDER_BLOCK_SIZE;
                let num_blocks_y = (data.row_end - data.row_start) / RENDER_BLOCK_SIZE;

                for local_block_y in 0..num_blocks_y {
                    for local_block_x in 0..num_blocks_x {
                        let block_size = RENDER_BLOCK_SIZE * RENDER_BLOCK_SIZE;
                        let start_pixel = (local_block_y * block_size * num_blocks_x)
                            + local_block_x * block_size;
                        let end_pixel = start_pixel + block_size;

                        let block_pixels = &pixels[(start_pixel as usize * NODE_BYTES_PER_PIXEL)
                            ..(end_pixel as usize * NODE_BYTES_PER_PIXEL)];

                        let image = turbojpeg::Image {
                            pixels: block_pixels,
                            width: RENDER_BLOCK_SIZE as usize,
                            height: RENDER_BLOCK_SIZE as usize,
                            pitch: RENDER_BLOCK_SIZE as usize * NODE_BYTES_PER_PIXEL,
                            format: NODE_PIXEL_FORMAT,
                        };
                        let compressed_pixel_bytes =
                            turbojpeg::compress(image, 95, turbojpeg::Subsamp::Sub2x2).unwrap();

                        let message =
                            NodeToHostMessage::RenderPartialFinished(RenderPartialFinishedData {
                                row: (local_block_y * RENDER_BLOCK_SIZE) + data.row_start,
                                column_block: local_block_x,
                                frame_idx: data.frame_idx,
                                compressed_pixel_bytes: compressed_pixel_bytes.to_vec(),
                            });

                        let mut addr = *addr;
                        addr.set_port(self.host_port);
                        self.socket
                            .packet_sender()
                            .send_unreliable(addr, message.to_bytes())
                            .unwrap();
                    }
                }

                self.socket.barrier().fetch_add(1, Ordering::SeqCst);
                self.socket
                    .packet_sender()
                    .send_barrier(*addr, vec![])
                    .unwrap();
            },
        );
    }

    pub fn run(mut self) {
        loop {
            #[allow(clippy::collapsible_match)]
            if let Ok(socket_event) = self.socket.event_receiver().try_recv() {
                match socket_event {
                    SocketEvent::Packet(packet, delay) => {
                        if delay > 0 {
                            continue;
                        }

                        if let Ok(message) = HostToNodeMessage::from_bytes(packet.payload()) {
                            match message {
                                HostToNodeMessage::StartRender(data) => {
                                    self.start_render(data, packet.addr());
                                }
                                HostToNodeMessage::VisibleWorldAction(data) => {
                                    let visible_world_action =
                                        VisibleWorldActionType::from_ty_and_bytes(
                                            data.ty,
                                            data.data.as_ref(),
                                        );

                                    self.renderer.visible_world_action(&visible_world_action);
                                }
                            }
                        } else {
                            log::warn!("Failed to read message from {}.", packet.addr());
                        }
                    }
                    SocketEvent::Connect(addr) => {
                        log::info!("Node connected at {:?}", addr);
                    }
                    SocketEvent::Disconnect(addr) => {
                        log::info!("Node disconnected at {:?}...", addr);
                    }
                }
            }

            thread::yield_now();
        }
    }
}
