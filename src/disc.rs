//! Reconstructs the flat `Token` stream from makemkv into a typed `Disc` hierarchy.
//!
//! Feed tokens to [`DiscBuilder::push`] one at a time (from an mpsc receiver, an iterator,
//! or any other source), then call [`DiscBuilder::finish`] to obtain a [`Disc`].
//! Tokens that don't describe disc structure (MSG, PRGV, etc.) are silently ignored.
//!
//! ```ignore
//! let mut builder = DiscBuilder::new();
//! while let Some(token) = rx.recv().await {
//!     builder.push(token);
//! }
//! let disc = builder.finish();
//! ```

use std::collections::HashMap;

use crate::parse::{AttributeId, Token};

type Attrs = HashMap<AttributeId, (u32, String)>;

/// Parsed disc-level metadata from `CINFO` tokens, plus all titles on the disc.
#[derive(Debug)]
pub struct Disc {
    pub name: Option<String>,
    pub volume_name: Option<String>,
    pub metadata_language_code: Option<String>,
    pub metadata_language_name: Option<String>,
    pub titles: Vec<Title>,
}

/// Parsed title metadata from `TINFO` tokens, plus all streams in that title.
#[derive(Debug)]
pub struct Title {
    pub name: Option<String>,
    pub chapter_count: Option<u32>,
    pub duration: Option<String>,
    pub disk_size: Option<String>,
    pub disk_size_bytes: Option<u64>,
    pub source_file_name: Option<String>,
    pub segments_count: Option<u32>,
    pub segments_map: Option<String>,
    pub output_file_name: Option<String>,
    pub metadata_language_code: Option<String>,
    pub metadata_language_name: Option<String>,
    pub streams: Vec<Stream>,
}

/// A parsed stream from `SINFO` tokens, discriminated by its `Type` attribute code.
/// Streams with an unrecognised type are dropped (logged in release, panicked in debug).
#[derive(Debug)]
pub enum Stream {
    Video(VideoStream),
    Audio(AudioStream),
    Subtitle(SubtitleStream),
}

#[derive(Debug)]
pub struct VideoStream {
    pub codec_id: Option<String>,
    pub codec_short: Option<String>,
    pub codec_long: Option<String>,
    pub size: Option<String>,
    pub aspect_ratio: Option<String>,
    pub frame_rate: Option<String>,
    pub flags: Option<u32>,
    pub metadata_language_code: Option<String>,
    pub metadata_language_name: Option<String>,
    pub output_conversion_type: Option<String>,
}

#[derive(Debug)]
pub struct AudioStream {
    pub name: Option<String>,
    pub lang_code: Option<String>,
    pub lang_name: Option<String>,
    pub codec_id: Option<String>,
    pub codec_short: Option<String>,
    pub codec_long: Option<String>,
    pub bitrate: Option<String>,
    pub channel_count: Option<u32>,
    pub sample_rate: Option<u32>,
    pub flags: Option<u32>,
    pub metadata_language_code: Option<String>,
    pub metadata_language_name: Option<String>,
    pub channel_layout: Option<String>,
    pub output_conversion_type: Option<String>,
}

#[derive(Debug)]
pub struct SubtitleStream {
    pub lang_code: Option<String>,
    pub lang_name: Option<String>,
    pub codec_id: Option<String>,
    pub codec_short: Option<String>,
    pub codec_long: Option<String>,
    pub flags: Option<u32>,
    pub metadata_language_code: Option<String>,
    pub metadata_language_name: Option<String>,
    pub output_conversion_type: Option<String>,
}

/// Accumulates `Token`s and constructs a [`Disc`] when [`finish`](DiscBuilder::finish) is called.
pub struct DiscBuilder {
    disc_attrs: Attrs,
    title_attrs: Vec<Attrs>,
    stream_attrs: Vec<Vec<Attrs>>,
}

impl DiscBuilder {
    pub fn new() -> Self {
        Self {
            disc_attrs: HashMap::new(),
            title_attrs: Vec::new(),
            stream_attrs: Vec::new(),
        }
    }

    /// Feed a token into the builder. `DiscAttribute`, `TitleAttribute`, and `StreamAttribute`
    /// tokens are accumulated; all others are ignored.
    pub fn push(&mut self, token: Token) {
        match token {
            Token::DiscAttribute { id, code, value } => {
                self.disc_attrs.insert(id, (code, value));
            }
            Token::TitleAttribute {
                title_index,
                id,
                code,
                value,
            } => {
                let ti = title_index as usize;
                if ti >= self.title_attrs.len() {
                    self.title_attrs.resize_with(ti + 1, HashMap::new);
                }
                self.title_attrs[ti].insert(id, (code, value));
            }
            Token::StreamAttribute {
                title_index,
                stream_index,
                id,
                code,
                value,
            } => {
                let ti = title_index as usize;
                let si = stream_index as usize;
                if ti >= self.stream_attrs.len() {
                    self.stream_attrs.resize_with(ti + 1, Vec::new);
                }
                if si >= self.stream_attrs[ti].len() {
                    self.stream_attrs[ti].resize_with(si + 1, HashMap::new);
                }
                self.stream_attrs[ti][si].insert(id, (code, value));
            }
            _ => {}
        }
    }

    /// Consume the builder and return the reconstructed [`Disc`].
    /// Streams without a recognised `Type` attribute are dropped.
    pub fn finish(self) -> Disc {
        let DiscBuilder {
            disc_attrs,
            mut title_attrs,
            mut stream_attrs,
        } = self;

        let n = title_attrs.len().max(stream_attrs.len());
        title_attrs.resize_with(n, HashMap::new);
        stream_attrs.resize_with(n, Vec::new);

        let titles = title_attrs
            .into_iter()
            .zip(stream_attrs)
            .map(|(tattrs, sattrs)| {
                let streams = sattrs.into_iter().filter_map(build_stream).collect();
                build_title(tattrs, streams)
            })
            .collect();

        Disc {
            name: get_str(&disc_attrs, AttributeId::Name),
            volume_name: get_str(&disc_attrs, AttributeId::VolumeName),
            metadata_language_code: get_str(&disc_attrs, AttributeId::MetadataLanguageCode),
            metadata_language_name: get_str(&disc_attrs, AttributeId::MetadataLanguageName),
            titles,
        }
    }
}

fn get_str(attrs: &Attrs, id: AttributeId) -> Option<String> {
    attrs.get(&id).map(|(_, v)| v.clone())
}

fn get_u32(attrs: &Attrs, id: AttributeId) -> Option<u32> {
    attrs.get(&id)?.1.parse().ok()
}

fn get_u64(attrs: &Attrs, id: AttributeId) -> Option<u64> {
    attrs.get(&id)?.1.parse().ok()
}

fn build_title(attrs: Attrs, streams: Vec<Stream>) -> Title {
    Title {
        name: get_str(&attrs, AttributeId::Name),
        chapter_count: get_u32(&attrs, AttributeId::ChapterCount),
        duration: get_str(&attrs, AttributeId::Duration),
        disk_size: get_str(&attrs, AttributeId::DiskSize),
        disk_size_bytes: get_u64(&attrs, AttributeId::DiskSizeBytes),
        source_file_name: get_str(&attrs, AttributeId::SourceFileName),
        segments_count: get_u32(&attrs, AttributeId::SegmentsCount),
        segments_map: get_str(&attrs, AttributeId::SegmentsMap),
        output_file_name: get_str(&attrs, AttributeId::OutputFileName),
        metadata_language_code: get_str(&attrs, AttributeId::MetadataLanguageCode),
        metadata_language_name: get_str(&attrs, AttributeId::MetadataLanguageName),
        streams,
    }
}

fn build_stream(attrs: Attrs) -> Option<Stream> {
    let &(stream_type_code, _) = attrs.get(&AttributeId::Type)?;
    match stream_type_code {
        6201 => Some(Stream::Video(VideoStream {
            codec_id: get_str(&attrs, AttributeId::CodecId),
            codec_short: get_str(&attrs, AttributeId::CodecShort),
            codec_long: get_str(&attrs, AttributeId::CodecLong),
            size: get_str(&attrs, AttributeId::VideoSize),
            aspect_ratio: get_str(&attrs, AttributeId::VideoAspectRatio),
            frame_rate: get_str(&attrs, AttributeId::VideoFrameRate),
            flags: get_u32(&attrs, AttributeId::StreamFlags),
            metadata_language_code: get_str(&attrs, AttributeId::MetadataLanguageCode),
            metadata_language_name: get_str(&attrs, AttributeId::MetadataLanguageName),
            output_conversion_type: get_str(&attrs, AttributeId::OutputConversionType),
        })),
        6202 => Some(Stream::Audio(AudioStream {
            name: get_str(&attrs, AttributeId::Name),
            lang_code: get_str(&attrs, AttributeId::LangCode),
            lang_name: get_str(&attrs, AttributeId::LangName),
            codec_id: get_str(&attrs, AttributeId::CodecId),
            codec_short: get_str(&attrs, AttributeId::CodecShort),
            codec_long: get_str(&attrs, AttributeId::CodecLong),
            bitrate: get_str(&attrs, AttributeId::Bitrate),
            channel_count: get_u32(&attrs, AttributeId::AudioChannelsCount),
            sample_rate: get_u32(&attrs, AttributeId::AudioSampleRate),
            flags: get_u32(&attrs, AttributeId::StreamFlags),
            metadata_language_code: get_str(&attrs, AttributeId::MetadataLanguageCode),
            metadata_language_name: get_str(&attrs, AttributeId::MetadataLanguageName),
            channel_layout: get_str(&attrs, AttributeId::AudioChannelLayoutName),
            output_conversion_type: get_str(&attrs, AttributeId::OutputConversionType),
        })),
        6203 => Some(Stream::Subtitle(SubtitleStream {
            lang_code: get_str(&attrs, AttributeId::LangCode),
            lang_name: get_str(&attrs, AttributeId::LangName),
            codec_id: get_str(&attrs, AttributeId::CodecId),
            codec_short: get_str(&attrs, AttributeId::CodecShort),
            codec_long: get_str(&attrs, AttributeId::CodecLong),
            flags: get_u32(&attrs, AttributeId::StreamFlags),
            metadata_language_code: get_str(&attrs, AttributeId::MetadataLanguageCode),
            metadata_language_name: get_str(&attrs, AttributeId::MetadataLanguageName),
            output_conversion_type: get_str(&attrs, AttributeId::OutputConversionType),
        })),
        code => {
            #[cfg(debug_assertions)]
            panic!("unknown stream type code {code}");
            #[cfg(not(debug_assertions))]
            {
                eprintln!("unknown stream type code {code}");
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{AttributeId, MsgFlags, Token, parse_line};

    fn disc_attr(id: AttributeId, value: &str) -> Token {
        Token::DiscAttribute {
            id,
            code: 0,
            value: value.into(),
        }
    }

    fn title_attr(ti: u32, id: AttributeId, value: &str) -> Token {
        Token::TitleAttribute {
            title_index: ti,
            id,
            code: 0,
            value: value.into(),
        }
    }

    fn stream_attr(ti: u32, si: u32, id: AttributeId, code: u32, value: &str) -> Token {
        Token::StreamAttribute {
            title_index: ti,
            stream_index: si,
            id,
            code,
            value: value.into(),
        }
    }

    fn build(tokens: impl IntoIterator<Item = Token>) -> Disc {
        let mut b = DiscBuilder::new();
        for t in tokens {
            b.push(t);
        }
        b.finish()
    }

    #[test]
    fn empty_disc() {
        let disc = DiscBuilder::new().finish();
        assert!(disc.name.is_none());
        assert!(disc.titles.is_empty());
    }

    #[test]
    fn disc_fields() {
        let disc = build([
            disc_attr(AttributeId::Name, "MyDisc"),
            disc_attr(AttributeId::VolumeName, "MYDISC"),
            disc_attr(AttributeId::MetadataLanguageCode, "eng"),
            disc_attr(AttributeId::MetadataLanguageName, "English"),
        ]);
        assert_eq!(disc.name.as_deref(), Some("MyDisc"));
        assert_eq!(disc.volume_name.as_deref(), Some("MYDISC"));
        assert_eq!(disc.metadata_language_code.as_deref(), Some("eng"));
        assert_eq!(disc.metadata_language_name.as_deref(), Some("English"));
        assert!(disc.titles.is_empty());
    }

    #[test]
    fn non_attribute_tokens_ignored() {
        let disc = build([
            Token::Message {
                code: 5011,
                flags: MsgFlags(0),
                count: 0,
                message: "ok".into(),
                format: "ok".into(),
                params: vec![],
            },
            Token::TitleCount { count: 6 },
            Token::ProgressValues {
                current: 100,
                total: 1000,
                max: 65536,
            },
        ]);
        assert!(disc.name.is_none());
        assert!(disc.titles.is_empty());
    }

    #[test]
    fn title_string_fields() {
        let disc = build([
            title_attr(0, AttributeId::Name, "MyDisc"),
            title_attr(0, AttributeId::Duration, "2:05:20"),
            title_attr(0, AttributeId::DiskSize, "77.2 GB"),
            title_attr(0, AttributeId::SourceFileName, "00801.mpls"),
            title_attr(0, AttributeId::OutputFileName, "MyDisc_t00.mkv"),
            title_attr(0, AttributeId::MetadataLanguageCode, "eng"),
            title_attr(0, AttributeId::MetadataLanguageName, "English"),
        ]);
        let t = &disc.titles[0];
        assert_eq!(t.name.as_deref(), Some("MyDisc"));
        assert_eq!(t.duration.as_deref(), Some("2:05:20"));
        assert_eq!(t.disk_size.as_deref(), Some("77.2 GB"));
        assert_eq!(t.source_file_name.as_deref(), Some("00801.mpls"));
        assert_eq!(t.output_file_name.as_deref(), Some("MyDisc_t00.mkv"));
        assert_eq!(t.metadata_language_code.as_deref(), Some("eng"));
    }

    #[test]
    fn title_numeric_fields() {
        let disc = build([
            title_attr(0, AttributeId::ChapterCount, "20"),
            title_attr(0, AttributeId::DiskSizeBytes, "82906933248"),
            title_attr(0, AttributeId::SegmentsCount, "1"),
        ]);
        let t = &disc.titles[0];
        assert_eq!(t.chapter_count, Some(20));
        assert_eq!(t.disk_size_bytes, Some(82906933248));
        assert_eq!(t.segments_count, Some(1));
    }

    #[test]
    fn title_segments_map_with_commas() {
        let disc = build([title_attr(
            0,
            AttributeId::SegmentsMap,
            "174,175,175,175,473",
        )]);
        assert_eq!(
            disc.titles[0].segments_map.as_deref(),
            Some("174,175,175,175,473")
        );
    }

    #[test]
    fn multiple_titles() {
        let disc = build([
            title_attr(0, AttributeId::Name, "Title 0"),
            title_attr(0, AttributeId::ChapterCount, "20"),
            title_attr(1, AttributeId::Name, "Title 1"),
            title_attr(1, AttributeId::ChapterCount, "5"),
            title_attr(2, AttributeId::Name, "Title 2"),
        ]);
        assert_eq!(disc.titles.len(), 3);
        assert_eq!(disc.titles[0].name.as_deref(), Some("Title 0"));
        assert_eq!(disc.titles[0].chapter_count, Some(20));
        assert_eq!(disc.titles[1].name.as_deref(), Some("Title 1"));
        assert_eq!(disc.titles[1].chapter_count, Some(5));
        assert_eq!(disc.titles[2].name.as_deref(), Some("Title 2"));
    }

    #[test]
    fn video_stream() {
        let disc = build([
            stream_attr(0, 0, AttributeId::Type, 6201, "Video"),
            stream_attr(0, 0, AttributeId::CodecId, 0, "V_MPEGH/ISO/HEVC"),
            stream_attr(0, 0, AttributeId::CodecShort, 0, "MpegH"),
            stream_attr(0, 0, AttributeId::CodecLong, 0, "MpegH HEVC Main10@L5.1"),
            stream_attr(0, 0, AttributeId::VideoSize, 0, "3840x2160"),
            stream_attr(0, 0, AttributeId::VideoAspectRatio, 0, "16:9"),
            stream_attr(
                0,
                0,
                AttributeId::VideoFrameRate,
                0,
                "23.976 (480000/20020)",
            ),
            stream_attr(0, 0, AttributeId::StreamFlags, 0, "65536"),
            stream_attr(0, 0, AttributeId::MetadataLanguageCode, 0, "eng"),
            stream_attr(0, 0, AttributeId::MetadataLanguageName, 0, "English"),
            stream_attr(
                0,
                0,
                AttributeId::OutputConversionType,
                5088,
                "( Lossless conversion )",
            ),
        ]);
        let Stream::Video(v) = &disc.titles[0].streams[0] else {
            panic!("expected video")
        };
        assert_eq!(v.codec_id.as_deref(), Some("V_MPEGH/ISO/HEVC"));
        assert_eq!(v.size.as_deref(), Some("3840x2160"));
        assert_eq!(v.aspect_ratio.as_deref(), Some("16:9"));
        assert_eq!(v.frame_rate.as_deref(), Some("23.976 (480000/20020)"));
        assert_eq!(v.flags, Some(65536));
        assert_eq!(v.metadata_language_code.as_deref(), Some("eng"));
        assert_eq!(
            v.output_conversion_type.as_deref(),
            Some("( Lossless conversion )")
        );
    }

    #[test]
    fn audio_stream_french_5_1() {
        let disc = build([
            stream_attr(0, 4, AttributeId::Type, 6202, "Audio"),
            stream_attr(0, 4, AttributeId::Name, 0, "Surround 5.1"),
            stream_attr(0, 4, AttributeId::LangCode, 0, "fra"),
            stream_attr(0, 4, AttributeId::LangName, 0, "French"),
            stream_attr(0, 4, AttributeId::CodecId, 0, "A_AC3"),
            stream_attr(0, 4, AttributeId::CodecShort, 0, "DD"),
            stream_attr(0, 4, AttributeId::CodecLong, 0, "Dolby Digital"),
            stream_attr(0, 4, AttributeId::Bitrate, 0, "640 Kb/s"),
            stream_attr(0, 4, AttributeId::AudioChannelsCount, 0, "6"),
            stream_attr(0, 4, AttributeId::AudioSampleRate, 0, "48000"),
            stream_attr(0, 4, AttributeId::StreamFlags, 0, "0"),
            stream_attr(0, 4, AttributeId::MetadataLanguageCode, 0, "eng"),
            stream_attr(0, 4, AttributeId::MetadataLanguageName, 0, "English"),
            stream_attr(0, 4, AttributeId::AudioChannelLayoutName, 0, "5.1(side)"),
            stream_attr(
                0,
                4,
                AttributeId::OutputConversionType,
                5088,
                "( Lossless conversion )",
            ),
        ]);
        // stream_index 4 means indices 0-3 are empty and filtered out by build_stream
        assert_eq!(disc.titles[0].streams.len(), 1);
        let Stream::Audio(a) = &disc.titles[0].streams[0] else {
            panic!("expected audio")
        };
        assert_eq!(a.name.as_deref(), Some("Surround 5.1"));
        assert_eq!(a.lang_code.as_deref(), Some("fra"));
        assert_eq!(a.lang_name.as_deref(), Some("French"));
        assert_eq!(a.codec_id.as_deref(), Some("A_AC3"));
        assert_eq!(a.codec_short.as_deref(), Some("DD"));
        assert_eq!(a.codec_long.as_deref(), Some("Dolby Digital"));
        assert_eq!(a.bitrate.as_deref(), Some("640 Kb/s"));
        assert_eq!(a.channel_count, Some(6));
        assert_eq!(a.sample_rate, Some(48000));
        assert_eq!(a.flags, Some(0));
        assert_eq!(a.channel_layout.as_deref(), Some("5.1(side)"));
        assert_eq!(
            a.output_conversion_type.as_deref(),
            Some("( Lossless conversion )")
        );
    }

    #[test]
    fn subtitle_stream() {
        let disc = build([
            stream_attr(0, 0, AttributeId::Type, 6203, "Subtitles"),
            stream_attr(0, 0, AttributeId::LangCode, 0, "eng"),
            stream_attr(0, 0, AttributeId::LangName, 0, "English"),
            stream_attr(0, 0, AttributeId::CodecId, 0, "S_HDMV/PGS"),
            stream_attr(0, 0, AttributeId::CodecShort, 0, "PGS"),
            stream_attr(0, 0, AttributeId::CodecLong, 0, "HDMV PGS Subtitles"),
            stream_attr(0, 0, AttributeId::StreamFlags, 0, "6144"),
            stream_attr(0, 0, AttributeId::MetadataLanguageCode, 0, "eng"),
            stream_attr(0, 0, AttributeId::MetadataLanguageName, 0, "English"),
            stream_attr(
                0,
                0,
                AttributeId::OutputConversionType,
                5088,
                "( Lossless conversion )",
            ),
        ]);
        let Stream::Subtitle(s) = &disc.titles[0].streams[0] else {
            panic!("expected subtitle")
        };
        assert_eq!(s.lang_code.as_deref(), Some("eng"));
        assert_eq!(s.codec_id.as_deref(), Some("S_HDMV/PGS"));
        assert_eq!(s.codec_long.as_deref(), Some("HDMV PGS Subtitles"));
        assert_eq!(s.flags, Some(6144));
        assert_eq!(
            s.output_conversion_type.as_deref(),
            Some("( Lossless conversion )")
        );
    }

    #[test]
    fn stream_missing_type_is_skipped() {
        let disc = build([stream_attr(0, 0, AttributeId::Name, 0, "orphan")]);
        assert!(disc.titles[0].streams.is_empty());
    }

    #[test]
    fn multiple_streams_mixed_types() {
        let disc = build([
            stream_attr(0, 0, AttributeId::Type, 6201, "Video"),
            stream_attr(0, 0, AttributeId::CodecId, 0, "V_MPEGH/ISO/HEVC"),
            stream_attr(0, 1, AttributeId::Type, 6202, "Audio"),
            stream_attr(0, 1, AttributeId::LangCode, 0, "eng"),
            stream_attr(0, 2, AttributeId::Type, 6203, "Subtitles"),
            stream_attr(0, 2, AttributeId::LangCode, 0, "fra"),
        ]);
        assert_eq!(disc.titles[0].streams.len(), 3);
        assert!(matches!(disc.titles[0].streams[0], Stream::Video(_)));
        assert!(matches!(disc.titles[0].streams[1], Stream::Audio(_)));
        assert!(matches!(disc.titles[0].streams[2], Stream::Subtitle(_)));
    }

    #[test]
    fn streams_across_multiple_titles() {
        let disc = build([
            stream_attr(0, 0, AttributeId::Type, 6201, "Video"),
            stream_attr(0, 1, AttributeId::Type, 6202, "Audio"),
            stream_attr(1, 0, AttributeId::Type, 6201, "Video"),
            stream_attr(1, 0, AttributeId::CodecId, 0, "V_MPEG2"),
        ]);
        assert_eq!(disc.titles[0].streams.len(), 2);
        assert_eq!(disc.titles[1].streams.len(), 1);
        let Stream::Video(v) = &disc.titles[1].streams[0] else {
            panic!()
        };
        assert_eq!(v.codec_id.as_deref(), Some("V_MPEG2"));
    }

    #[test]
    fn build_from_parsed_lines() {
        let lines = [
            r#"CINFO:2,0,"MyDisc""#,
            r#"CINFO:32,0,"MYDISC""#,
            r#"TINFO:0,2,0,"MyDisc""#,
            r#"TINFO:0,8,0,"20""#,
            r#"SINFO:0,0,1,6201,"Video""#,
            r#"SINFO:0,0,5,0,"V_MPEGH/ISO/HEVC""#,
            r#"SINFO:0,1,1,6202,"Audio""#,
            r#"SINFO:0,1,3,0,"eng""#,
            r#"SINFO:0,1,14,0,"8""#,
        ];
        let disc = build(lines.iter().filter_map(|l| parse_line(l).ok()));
        assert_eq!(disc.name.as_deref(), Some("MyDisc"));
        assert_eq!(disc.volume_name.as_deref(), Some("MYDISC"));
        assert_eq!(disc.titles[0].name.as_deref(), Some("MyDisc"));
        assert_eq!(disc.titles[0].chapter_count, Some(20));
        assert_eq!(disc.titles[0].streams.len(), 2);
        assert!(matches!(disc.titles[0].streams[0], Stream::Video(_)));
        let Stream::Audio(a) = &disc.titles[0].streams[1] else {
            panic!()
        };
        assert_eq!(a.lang_code.as_deref(), Some("eng"));
        assert_eq!(a.channel_count, Some(8));
    }
}
