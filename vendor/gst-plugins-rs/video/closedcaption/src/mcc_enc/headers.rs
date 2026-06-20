// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

pub const PREAMBLE_V1: &[u8] = b"File Format=MacCaption_MCC V1.0\r\n\
\r\n\
///////////////////////////////////////////////////////////////////////////////////\r\n\
// Computer Prompting and Captioning Company\r\n\
// Ancillary Data Packet Transfer File\r\n\
//\r\n\
// Permission to generate this format is granted provided that\r\n\
//   1. This ANC Transfer file format is used on an as-is basis and no warranty is given, and\r\n\
//   2. This entire descriptive information text is included in a generated .mcc file.\r\n\
//\r\n\
// General file format:\r\n\
//   HH:MM:SS:FF(tab)[Hexadecimal ANC data in groups of 2 characters]\r\n\
//     Hexadecimal data starts with the Ancillary Data Packet DID (Data ID defined in S291M)\r\n\
//       and concludes with the Check Sum following the User Data Words.\r\n\
//     Each time code line must contain at most one complete ancillary data packet.\r\n\
//     To transfer additional ANC Data successive lines may contain identical time code.\r\n\
//     Time Code Rate=[24, 25, 30, 30DF, 50, 60]\r\n\
//\r\n\
//   ANC data bytes may be represented by one ASCII character according to the following schema:\r\n\
//     G  FAh 00h 00h\r\n\
//     H  2 x (FAh 00h 00h)\r\n\
//     I  3 x (FAh 00h 00h)\r\n\
//     J  4 x (FAh 00h 00h)\r\n\
//     K  5 x (FAh 00h 00h)\r\n\
//     L  6 x (FAh 00h 00h)\r\n\
//     M  7 x (FAh 00h 00h)\r\n\
//     N  8 x (FAh 00h 00h)\r\n\
//     O  9 x (FAh 00h 00h)\r\n\
//     P  FBh 80h 80h\r\n\
//     Q  FCh 80h 80h\r\n\
//     R  FDh 80h 80h\r\n\
//     S  96h 69h\r\n\
//     T  61h 01h\r\n\
//     U  E1h 00h 00h 00h\r\n\
//     Z  00h\r\n\
//\r\n\
///////////////////////////////////////////////////////////////////////////////////\r\n\
\r\n";

pub const PREAMBLE_V2: &[u8] = b"File Format=MacCaption_MCC V2.0\r\n\
\r\n\
///////////////////////////////////////////////////////////////////////////////////\r\n\
// Computer Prompting and Captioning Company\r\n\
// Ancillary Data Packet Transfer File\r\n\
//\r\n\
// Permission to generate this format is granted provided that\r\n\
//   1. This ANC Transfer file format is used on an as-is basis and no warranty is given, and\r\n\
//   2. This entire descriptive information text is included in a generated .mcc file.\r\n\
//\r\n\
// General file format:\r\n\
//   HH:MM:SS:FF(tab)[Hexadecimal ANC data in groups of 2 characters]\r\n\
//     Hexadecimal data starts with the Ancillary Data Packet DID (Data ID defined in S291M)\r\n\
//       and concludes with the Check Sum following the User Data Words.\r\n\
//     Each time code line must contain at most one complete ancillary data packet.\r\n\
//     To transfer additional ANC Data successive lines may contain identical time code.\r\n\
//     Time Code Rate=[24, 25, 30, 30DF, 50, 60, 60DF]\r\n\
//     Time Code Rate=[24, 25, 30, 30DF, 50, 60]\r\n\
//\r\n\
//   ANC data bytes may be represented by one ASCII character according to the following schema:\r\n\
//     G  FAh 00h 00h\r\n\
//     H  2 x (FAh 00h 00h)\r\n\
//     I  3 x (FAh 00h 00h)\r\n\
//     J  4 x (FAh 00h 00h)\r\n\
//     K  5 x (FAh 00h 00h)\r\n\
//     L  6 x (FAh 00h 00h)\r\n\
//     M  7 x (FAh 00h 00h)\r\n\
//     N  8 x (FAh 00h 00h)\r\n\
//     O  9 x (FAh 00h 00h)\r\n\
//     P  FBh 80h 80h\r\n\
//     Q  FCh 80h 80h\r\n\
//     R  FDh 80h 80h\r\n\
//     S  96h 69h\r\n\
//     T  61h 01h\r\n\
//     U  E1h 00h 00h 00h\r\n\
//     Z  00h\r\n\
//\r\n\
///////////////////////////////////////////////////////////////////////////////////\r\n\
\r\n";
