use core::{net::SocketAddr, ops::FnMut, sync::atomic::Ordering};
use std::thread;

use anyhow::Result;
use appearance_world::visible_world_action::VisibleWorldActionType;
use unreliable::{Socket, SocketEvent};

use crate::host::{
    HostToNodeMessage, NodeToHostMessage, RenderPartialFinishedData, StartRenderData,
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
                let max_pixels_per_package = 20; //(508 - 12) / 4;

                let packages_per_row = data.width.div_ceil(max_pixels_per_package);

                let packet_sender = self.socket.packet_sender();

                for local_row in 0..(data.row_end - data.row_start) {
                    let row = local_row + data.row_start;

                    let mut pixels_processed_this_row = 0;
                    for i in 0..packages_per_row {
                        let first_pixel_in_row = i * max_pixels_per_package;
                        let num_pixels_in_row = if i < packages_per_row - 1 {
                            max_pixels_per_package
                        } else {
                            data.width - pixels_processed_this_row
                        };
                        pixels_processed_this_row += num_pixels_in_row;

                        let pixel_start = local_row * data.width + first_pixel_in_row;
                        let pixel_end = pixel_start + num_pixels_in_row;

                        let pixel_row =
                            pixels[(pixel_start * 4) as usize..(pixel_end * 4) as usize].to_vec();

                        let message =
                            NodeToHostMessage::RenderPartialFinished(RenderPartialFinishedData {
                                row,
                                row_start: first_pixel_in_row,
                                pixels: pixel_row,
                            });

                        packet_sender
                            .send_unreliable(*addr, message.to_bytes())
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
