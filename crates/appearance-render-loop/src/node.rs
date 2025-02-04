use core::{net::SocketAddr, ops::FnMut, sync::atomic::Ordering};
use std::thread;

use anyhow::Result;
use appearance_time::Timer;
use appearance_world::visible_world_action::VisibleWorldActionType;
use unreliable::{Socket, SocketEvent, MAX_PACKET_PAYLOAD_SIZE};

use crate::host::{
    HostToNodeMessage, NodeToHostMessage, RenderPartialFinishedData, StartRenderData,
    RENDER_BLOCK_SIZE,
};

pub trait NodeRenderer {
    // TODO: world manipulation
    fn visible_world_action(&mut self, action: &VisibleWorldActionType);

    fn render<F: FnMut(&[u8])>(
        &mut self,
        width: u32,
        height: u32,
        start_row: u32,
        end_row: u32,
        result_callback: F,
    );
}

pub struct Node<T: NodeRenderer> {
    socket: Socket,
    renderer: T,
}

impl<T: NodeRenderer + 'static> Node<T> {
    pub fn new(renderer: T, host_addr: SocketAddr) -> Result<Self> {
        let socket = Socket::new(host_addr)?;

        Ok(Self { socket, renderer })
    }

    fn start_render(&mut self, data: StartRenderData, addr: &SocketAddr) {
        log::info!("start render: {:?}", data);

        self.renderer.render(
            data.width,
            data.height,
            data.row_start,
            data.row_end,
            |pixels| {
                //let timer = Timer::new();'

                let num_blocks_x = data.width / RENDER_BLOCK_SIZE;
                let num_blocks_y = (data.row_end - data.row_start) / RENDER_BLOCK_SIZE;

                for local_block_y in 0..num_blocks_y {
                    for local_block_x in 0..num_blocks_x {
                        // TODO: order pixels already in blocks inside the renderer
                        // let mut block_pixels =
                        //     vec![0u8; (RENDER_BLOCK_SIZE * RENDER_BLOCK_SIZE * 4) as usize];
                        // for local_y in 0..RENDER_BLOCK_SIZE {
                        //     for local_x in 0..RENDER_BLOCK_SIZE {
                        //         let y = local_y + (local_block_y * RENDER_BLOCK_SIZE);
                        //         let x = local_x + (local_block_x * RENDER_BLOCK_SIZE);

                        //         let id = (y * data.width + x) as usize;
                        //         let local_id = (local_y * RENDER_BLOCK_SIZE + local_x) as usize;

                        //         for i in 0..4 {
                        //             block_pixels[local_id * 4 + i] = pixels[id * 4 + i];
                        //         }
                        //     }
                        // }

                        let block_size = RENDER_BLOCK_SIZE * RENDER_BLOCK_SIZE;
                        let start_pixel = (local_block_y * block_size * num_blocks_x)
                            + local_block_x * block_size;
                        let end_pixel = start_pixel + block_size;

                        let block_pixels =
                            &pixels[(start_pixel * 4) as usize..(end_pixel * 4) as usize];

                        // TODO: compress
                        let image = turbojpeg::Image {
                            pixels: block_pixels,
                            width: RENDER_BLOCK_SIZE as usize,
                            height: RENDER_BLOCK_SIZE as usize,
                            pitch: RENDER_BLOCK_SIZE as usize * 4,
                            format: turbojpeg::PixelFormat::RGBA,
                        };
                        let compressed_pixel_bytes =
                            turbojpeg::compress(image, 95, turbojpeg::Subsamp::Sub2x2).unwrap();

                        //let compressed_pixels = block_pixels;

                        let message =
                            NodeToHostMessage::RenderPartialFinished(RenderPartialFinishedData {
                                row: (local_block_y * RENDER_BLOCK_SIZE) + data.row_start,
                                column_block: local_block_x,
                                compressed_pixel_bytes: compressed_pixel_bytes.to_vec(),
                            });

                        self.socket
                            .packet_sender()
                            .send_unreliable(*addr, message.to_bytes())
                            .unwrap();
                    }
                }

                // let max_pixels_per_package = (MAX_PACKET_PAYLOAD_SIZE as u32 - 12) / 4;

                // let packages_per_row = data.width.div_ceil(max_pixels_per_package);

                // let packet_sender = self.socket.packet_sender();

                // for local_row in 0..(data.row_end - data.row_start) {
                //     let row = local_row + data.row_start;

                //     let mut pixels_processed_this_row = 0;
                //     for i in 0..packages_per_row {
                //         let first_pixel_in_row = i * max_pixels_per_package;
                //         let num_pixels_in_row = if i < packages_per_row - 1 {
                //             max_pixels_per_package
                //         } else {
                //             data.width - pixels_processed_this_row
                //         };
                //         pixels_processed_this_row += num_pixels_in_row;

                //         let pixel_start = local_row * data.width + first_pixel_in_row;
                //         let pixel_end = pixel_start + num_pixels_in_row;

                //         let pixel_row =
                //             pixels[(pixel_start * 4) as usize..(pixel_end * 4) as usize].to_vec();

                //         let message =
                //             NodeToHostMessage::RenderPartialFinished(RenderPartialFinishedData {
                //                 row,
                //                 row_start: first_pixel_in_row,
                //                 pixels: pixel_row,
                //             });

                //         packet_sender
                //             .send_unreliable(*addr, message.to_bytes())
                //             .unwrap();
                //     }
                // }

                //println!("Took {}ms", timer.elapsed() * 1000.0);

                self.socket.barrier().fetch_add(1, Ordering::Relaxed);
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
                if let SocketEvent::Packet(packet) = socket_event {
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
            }

            thread::yield_now();
        }
    }
}
