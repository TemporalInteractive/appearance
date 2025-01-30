use core::{
    iter::{IntoIterator, Iterator},
    net::SocketAddr,
};
use std::thread;

use rayon::prelude::*;

use anyhow::Result;
use appearance_time::Timer;
use appearance_world::visible_world_action::VisibleWorldActionType;
use crossbeam::channel::{Receiver, Sender};
use laminar::{Packet, Socket, SocketEvent};

use crate::host::{
    laminar_config, HostToNodeMessage, NodeToHostMessage, RenderFinishedData,
    RenderPartialFinishedData,
};

pub trait NodeRenderer {
    // TODO: world manipulation
    fn visible_world_action(&mut self, action: &VisibleWorldActionType);

    fn render<F: Fn(&[u8])>(
        &mut self,
        width: u32,
        height: u32,
        start_row: u32,
        end_row: u32,
        result_callback: F,
    );
}

pub struct Node<T: NodeRenderer> {
    packet_sender: Sender<Packet>,
    event_receiver: Receiver<SocketEvent>,
    renderer: T,
    host_addr: SocketAddr,
    connection_timer: Timer,
}

fn sum_of_squares(input: &[u32]) -> u32 {
    input
        .par_iter() // <-- just change that!
        .map(|&i| i * i)
        .sum()
}

impl<T: NodeRenderer + 'static> Node<T> {
    pub fn new(renderer: T, host_ip: &str, host_port: &str, node_port: &str) -> Result<Self> {
        let mut socket =
            Socket::bind_with_config(format!("0.0.0.0:{}", node_port), laminar_config())?;
        let packet_sender = socket.get_packet_sender();
        let event_receiver = socket.get_event_receiver();
        thread::spawn(move || socket.start_polling());

        Ok(Self {
            event_receiver,
            packet_sender,
            renderer,
            host_addr: format!("{}:{}", host_ip, host_port).parse().unwrap(),
            connection_timer: Timer::new(),
        })
    }

    pub fn run(mut self) {
        loop {
            // Keep trying to connect every second
            if self.connection_timer.elapsed() > 1.0 {
                let message = NodeToHostMessage::Connect;
                let packet = Packet::unreliable(self.host_addr, message.to_bytes());
                self.packet_sender.send(packet).unwrap();

                self.connection_timer.reset();
            }

            #[allow(clippy::collapsible_match)]
            if let Ok(socket_event) = self.event_receiver.try_recv() {
                if let SocketEvent::Packet(packet) = socket_event {
                    if let Ok(message) = HostToNodeMessage::from_bytes(packet.payload()) {
                        match message {
                            HostToNodeMessage::StartRender(data) => {
                                log::info!("start render: {:?}", data);

                                self.renderer.render(
                                    data.width,
                                    data.height,
                                    data.row_start,
                                    data.row_end,
                                    |pixels| {
                                        let max_pixels_per_package = 20; //(508 - 12) / 4;
                                                                         // TODO: why does this calc not work??

                                        let packages_per_row =
                                            data.width.div_ceil(max_pixels_per_package);

                                        let packets_vec = (0..(data.row_end - data.row_start))
                                            .collect::<Vec<u32>>()
                                            .par_iter()
                                            .map(|local_row| {
                                                let row = local_row + data.row_start;
                                                let mut packets = vec![];

                                                let mut pixels_processed_this_row = 0;
                                                for i in 0..packages_per_row {
                                                    let first_pixel_in_row =
                                                        i * max_pixels_per_package;
                                                    let num_pixels_in_row =
                                                        if i < packages_per_row - 1 {
                                                            max_pixels_per_package
                                                        } else {
                                                            data.width - pixels_processed_this_row
                                                        };
                                                    pixels_processed_this_row += num_pixels_in_row;

                                                    let pixel_start =
                                                        local_row * data.width + first_pixel_in_row;
                                                    let pixel_end = pixel_start + num_pixels_in_row;

                                                    let pixel_row = pixels[(pixel_start * 4)
                                                        as usize
                                                        ..(pixel_end * 4) as usize]
                                                        .to_vec();

                                                    let message =
                                                        NodeToHostMessage::RenderPartialFinished(
                                                            RenderPartialFinishedData {
                                                                row,
                                                                row_start: first_pixel_in_row,
                                                                pixels: pixel_row,
                                                            },
                                                        );
                                                    packets.push(Packet::unreliable_sequenced(
                                                        packet.addr(),
                                                        message.to_bytes(),
                                                        None,
                                                    ));
                                                }

                                                packets
                                            })
                                            .collect::<Vec<_>>();

                                        {
                                            let message = NodeToHostMessage::RenderFinished(
                                                RenderFinishedData {
                                                    frame_idx: data.frame_idx,
                                                },
                                            );
                                            let packet = Packet::reliable_ordered(
                                                packet.addr(),
                                                message.to_bytes(),
                                                None,
                                            );
                                            self.packet_sender.send(packet).unwrap();
                                        }

                                        for packets in packets_vec {
                                            for packet in packets {
                                                self.packet_sender.send(packet).unwrap();
                                            }
                                        }
                                    },
                                );
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
                        //log::warn!("Failed to read message from {}.", packet.addr());
                    }
                }
            }

            thread::yield_now();
        }
    }
}
