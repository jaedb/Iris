## Introduction
This is GStreamer implementation of RaptorQ FEC for RTP streams.

The sender element produces requested number `X` of repair packets from `K` RTP
packets.  The receiver only needs:

- `K` of any repair or RTP packets to recover all the data with 99% probability
- `K + 1` of any repair or RTP packets to recover all the data with 99.99%
  probability,
- `K + 2` of any repair or RTP packets to recover all the data with 99.9999%
  probability etc.

Relevant documents:
- [RFC6363 - Forward Error Correction (FEC) Framework](https://datatracker.ietf.org/doc/html/rfc6363)
- [RFC6681 - Raptor Forward Error Correction (FEC) Schemes for FECFRAME](https://datatracker.ietf.org/doc/html/rfc6681)
- [RFC6682 - RTP Payload Format for Raptor Forward Error Correction (FEC)](https://datatracker.ietf.org/doc/html/rfc6682)


## Sender/Receiver Example
```shell
    gst-launch-1.0 \
        rtpbin name=rtp fec-encoders='fec,0="raptorqenc\ mtu=1356\ symbol-size=192";' \
        uridecodebin uri=file:///path/to/video/file ! x264enc key-int-max=60 tune=zerolatency ! \
          queue ! mpegtsmux ! rtpmp2tpay ssrc=0 ! \
        rtp.send_rtp_sink_0 rtp.send_rtp_src_0 ! udpsink host=127.0.0.1 port=5000 \
        rtp.send_fec_src_0_0 ! udpsink host=127.0.0.1 port=5002 async=false

    gst-launch-1.0 \
        rtpbin latency=200 fec-decoders='fec,0="raptorqdec";' name=rtp \
        udpsrc address=127.0.0.1 port=5002 \
          caps="application/x-rtp, payload=96, raptor-scheme-id=(string)6, repair-window=(string)1000000, t=(string)192" ! \
        queue ! rtp.recv_fec_sink_0_0 \
        udpsrc address=127.0.0.1 port=5000 \
           caps="application/x-rtp, media=video, clock-rate=90000, encoding-name=mp2t, payload=33" ! \
        queue ! netsim drop-probability=0.05 ! rtp.recv_rtp_sink_0 \
        rtp. ! decodebin ! videoconvert ! queue ! autovideosink
```

## Implementation Details

### Encoder Element
The encoder element stores the copy of original RTP packets internally until it
receives the number of packets that are requested to be protected together. At
this point it creates a Source Block that is passed to RaptorQ Encoder. Source
Block is constructed by concatenating ADUIs (Application Data Unit Information)
sometimes also called SPI (Source Packet Information). Each ADUI contains:

- Header with Flow ID - `F(I)` and Length Indication for the packet - `L(I)`,
- UDP payload, this a complete RTP packet with header,
- Padding bytes if required,

```text
            T                T                T                T
    <----------------><--------------><---------------><---------------->
    +----+--------+-----------------------+-----------------------------+
    |F[0]|  L[0]  |        ADU[0]         |            Pad[0]           |
    +----+--------+----------+------------+-----------------------------+
    |F[1]|  L[1]  | ADU[1]   |                         Pad[1]           |
    +----+--------+----------+------------------------------------------+
    |F[2]|  L[2]  |                    ADU[2]                           |
    +----+--------+------+----------------------------------------------+
    |F[3]|  L[3]  |ADU[3]|                             Pad[3]           |
    +----+--------+------+----------------------------------------------+
    \_________________________________  ________________________________/
                                      \/
                             RaptorQ FEC encoding
    
    +-------------------------------------------------------------------+
    |                              Repair 4                             |
    +-------------------------------------------------------------------+
    .                                                                   .
    .                                                                   .
    +-------------------------------------------------------------------+
    |                              Repair 7                             |
    +-------------------------------------------------------------------+
    
    T - Symbol Size
    F - Flow ID
    L - Length Indication
    ADU - Application Data Unit (RTP packet)
```

Encoder element creates requested number of packets for a given Source Block.
The repair packets are send during `repair-window` which is configurable
parameter. E.g. if encoder element produces 5 repair packets and `repair-window`
is set to 500ms, a first repair packet is send 100ms after the last protected
packet, second at 200ms and the last at `repair-window`.

Each repair packet except the symbols that are required to recover missing
source packets, contains also the information about the Source Block:

-   `I` - Initial sequence number of the Source Block,
-   `Lp` - ADUI length in symbols,
-   `Lb` - Source Block Length in symbols,

### Decoder Element
Decoder element stores the copy of received RTP packets, and push original
packet downstream immediately. If all the RTP packets have been received, the
buffered media packets are dropped. If any packets are missing, the receiver
checks if it has enough buffered media and repair packets to perform decoding.
If that's the case it tries to recover missing packets by building the Source
Block following the same rules as sender, except it skips missing packets and
append repair packets to the block instead.

Because the receiver element does not introduce latency, the recovered packets
are send out of sequence, and it requires a `rtpjitterbuffer` to be chained
downstream. The `rtpjitterbuffer` needs to be configured with high enough
latency.

The receiver to determine which media packets belongs to Source Blocks uses the
information that can be retrieved from any of the repair packets. Then media
packets with Sequence Numbers: `I + Lb/Lp - 1` inclusive, are considered during
building a Source Block.

The receiver uses `repair-window` that is signaled by the sender, and its own
`repair-window-tolerance` parameter to decide for how long it should wait for
the corresponding repair packets before giving up. The wait time is
`repair-window + repair-window-tolerance`.
