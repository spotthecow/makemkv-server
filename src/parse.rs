#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("not a token")]
    NotAToken,
    #[error("unknown token kind '{0}'")]
    UnknownKind(String),
    #[error("missing field")]
    MissingField,
    #[error("invalid integer")]
    InvalidInt(#[from] std::num::ParseIntError),
    #[error("unterminated quoted string")]
    UnterminatedString,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttributeId {
    Unknown,
    Type,
    Name,
    LangCode,
    LangName,
    CodecId,
    CodecShort,
    CodecLong,
    ChapterCount,
    Duration,
    DiskSize,
    DiskSizeBytes,
    StreamTypeExtension,
    Bitrate,
    AudioChannelsCount,
    AngleInfo,
    SourceFileName,
    AudioSampleRate,
    AudioSampleSize,
    VideoSize,
    VideoAspectRatio,
    VideoFrameRate,
    StreamFlags,
    DateTime,
    OriginalTitleId,
    SegmentsCount,
    SegmentsMap,
    OutputFileName,
    MetadataLanguageCode,
    MetadataLanguageName,
    TreeInfo,
    PanelTitle,
    VolumeName,
    OrderWeight,
    OutputFormat,
    OutputFormatDescription,
    SeamlessInfo,
    PanelText,
    MkvFlags,
    MkvFlagsText,
    AudioChannelLayoutName,
    OutputCodecShort,
    OutputConversionType,
    OutputAudioSampleRate,
    OutputAudioSampleSize,
    OutputAudioChannelsCount,
    OutputAudioChannelLayoutName,
    OutputAudioChannelLayout,
    OutputAudioMixDescription,
    Comment,
    OffsetSequenceId,
    Other(u32),
}

impl From<u32> for AttributeId {
    fn from(n: u32) -> Self {
        match n {
            0  => Self::Unknown,
            1  => Self::Type,
            2  => Self::Name,
            3  => Self::LangCode,
            4  => Self::LangName,
            5  => Self::CodecId,
            6  => Self::CodecShort,
            7  => Self::CodecLong,
            8  => Self::ChapterCount,
            9  => Self::Duration,
            10 => Self::DiskSize,
            11 => Self::DiskSizeBytes,
            12 => Self::StreamTypeExtension,
            13 => Self::Bitrate,
            14 => Self::AudioChannelsCount,
            15 => Self::AngleInfo,
            16 => Self::SourceFileName,
            17 => Self::AudioSampleRate,
            18 => Self::AudioSampleSize,
            19 => Self::VideoSize,
            20 => Self::VideoAspectRatio,
            21 => Self::VideoFrameRate,
            22 => Self::StreamFlags,
            23 => Self::DateTime,
            24 => Self::OriginalTitleId,
            25 => Self::SegmentsCount,
            26 => Self::SegmentsMap,
            27 => Self::OutputFileName,
            28 => Self::MetadataLanguageCode,
            29 => Self::MetadataLanguageName,
            30 => Self::TreeInfo,
            31 => Self::PanelTitle,
            32 => Self::VolumeName,
            33 => Self::OrderWeight,
            34 => Self::OutputFormat,
            35 => Self::OutputFormatDescription,
            36 => Self::SeamlessInfo,
            37 => Self::PanelText,
            38 => Self::MkvFlags,
            39 => Self::MkvFlagsText,
            40 => Self::AudioChannelLayoutName,
            41 => Self::OutputCodecShort,
            42 => Self::OutputConversionType,
            43 => Self::OutputAudioSampleRate,
            44 => Self::OutputAudioSampleSize,
            45 => Self::OutputAudioChannelsCount,
            46 => Self::OutputAudioChannelLayoutName,
            47 => Self::OutputAudioChannelLayout,
            48 => Self::OutputAudioMixDescription,
            49 => Self::Comment,
            50 => Self::OffsetSequenceId,
            _  => Self::Other(n),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MsgFlags(pub u32);

impl MsgFlags {
    pub fn is_debug(&self) -> bool { self.0 & 32 != 0 }
    pub fn is_hidden(&self) -> bool { self.0 & 64 != 0 }
    pub fn is_event(&self) -> bool { self.0 & 128 != 0 }
    pub fn requires_response(&self) -> bool { self.0 & 3854 != 0 }
    pub fn is_error(&self) -> bool { self.0 & 3854 == 516 }
    pub fn has_url(&self) -> bool { self.0 & 131072 != 0 }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskFlags(pub u32);

impl DiskFlags {
    pub fn has_dvd(&self) -> bool { self.0 & 1 != 0 }
    pub fn has_hdvd(&self) -> bool { self.0 & 2 != 0 }
    pub fn has_bluray(&self) -> bool { self.0 & 4 != 0 }
    pub fn has_aacs(&self) -> bool { self.0 & 8 != 0 }
    pub fn has_bdsvm(&self) -> bool { self.0 & 16 != 0 }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Message {
        code: u32,
        flags: MsgFlags,
        count: u32,
        message: String,
        format: String,
        params: Vec<String>,
    },
    ProgressCurrentTitle { code: u32, id: u32, name: String },
    ProgressTotalTitle { code: u32, id: u32, name: String },
    ProgressValues { current: u32, total: u32, max: u32 },
    Drive {
        index: u32,
        visible: bool,
        enabled: bool,
        flags: DiskFlags,
        drive_name: String,
        disc_name: String,
    },
    TitleCount { count: u32 },
    DiscAttribute { id: AttributeId, code: u32, value: String },
    TitleAttribute { title_index: u32, id: AttributeId, code: u32, value: String },
    StreamAttribute { title_index: u32, stream_index: u32, id: AttributeId, code: u32, value: String },
}

pub struct Parser<'a> {
    rest: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(s: &'a str) -> Self {
        Self { rest: s.trim_end() }
    }

    pub fn int(&mut self) -> Result<i32, ParseError> {
        let (field, rest) = self.rest.split_once(',').unwrap_or((self.rest, ""));
        self.rest = rest;
        Ok(field.parse()?)
    }

    pub fn uint(&mut self) -> Result<u32, ParseError> {
        let (field, rest) = self.rest.split_once(',').unwrap_or((self.rest, ""));
        self.rest = rest;
        Ok(field.parse()?)
    }

    pub fn bool_flag(&mut self) -> Result<bool, ParseError> {
        Ok(self.uint()? != 0)
    }

    pub fn string(&mut self) -> Result<String, ParseError> {
        let s = self.rest.strip_prefix('"').ok_or(ParseError::MissingField)?;
        let mut field = String::new();
        let mut chars = s.char_indices();
        let end = loop {
            match chars.next() {
                None => return Err(ParseError::UnterminatedString),
                Some((i, '"')) => break i,
                Some((_, '\\')) => match chars.next() {
                    Some((_, '"'))  => field.push('"'),
                    Some((_, '\\')) => field.push('\\'),
                    Some((_, c))    => { field.push('\\'); field.push(c); }
                    None => return Err(ParseError::UnterminatedString),
                },
                Some((_, c)) => field.push(c),
            }
        };
        self.rest = s[end + 1..].strip_prefix(',').unwrap_or(&s[end + 1..]);
        Ok(field)
    }

    pub fn string_params(&mut self) -> Result<Vec<String>, ParseError> {
        let mut params = Vec::new();
        while !self.rest.is_empty() {
            params.push(self.string()?);
        }
        Ok(params)
    }
}

pub fn parse_line(line: &str) -> Result<Token, ParseError> {
    let (kind, rest) = line.split_once(':').ok_or(ParseError::NotAToken)?;
    let mut p = Parser::new(rest);

    match kind {
        "MSG" => Ok(Token::Message {
            code: p.uint()?,
            flags: MsgFlags(p.uint()?),
            count: p.uint()?,
            message: p.string()?,
            format: p.string()?,
            params: p.string_params()?,
        }),
        "PRGC" => Ok(Token::ProgressCurrentTitle {
            code: p.uint()?,
            id: p.uint()?,
            name: p.string()?,
        }),
        "PRGT" => Ok(Token::ProgressTotalTitle {
            code: p.uint()?,
            id: p.uint()?,
            name: p.string()?,
        }),
        "PRGV" => Ok(Token::ProgressValues {
            current: p.uint()?,
            total: p.uint()?,
            max: p.uint()?,
        }),
        "DRV" => Ok(Token::Drive {
            index: p.uint()?,
            visible: p.bool_flag()?,
            enabled: p.bool_flag()?,
            flags: DiskFlags(p.uint()?),
            drive_name: p.string()?,
            disc_name: p.string()?,
        }),
        "TCOUNT" => Ok(Token::TitleCount { count: p.uint()? }),
        "CINFO" => Ok(Token::DiscAttribute {
            id: AttributeId::from(p.uint()?),
            code: p.uint()?,
            value: p.string()?,
        }),
        "TINFO" => Ok(Token::TitleAttribute {
            title_index: p.uint()?,
            id: AttributeId::from(p.uint()?),
            code: p.uint()?,
            value: p.string()?,
        }),
        "SINFO" => Ok(Token::StreamAttribute {
            title_index: p.uint()?,
            stream_index: p.uint()?,
            id: AttributeId::from(p.uint()?),
            code: p.uint()?,
            value: p.string()?,
        }),
        _ => Err(ParseError::UnknownKind(kind.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Token {
        parse_line(s).expect("parse failed")
    }

    #[test]
    fn msg_no_params() {
        assert_eq!(
            parse(r#"MSG:5011,0,0,"Operation successfully completed","Operation successfully completed""#),
            Token::Message {
                code: 5011,
                flags: MsgFlags(0),
                count: 0,
                message: "Operation successfully completed".into(),
                format: "Operation successfully completed".into(),
                params: vec![],
            }
        );
    }

    #[test]
    fn msg_with_params() {
        assert_eq!(
            parse(r#"MSG:1234,0,2,"File not found","File %1 not found","foo.mkv","bar""#),
            Token::Message {
                code: 1234,
                flags: MsgFlags(0),
                count: 2,
                message: "File not found".into(),
                format: "File %1 not found".into(),
                params: vec!["foo.mkv".into(), "bar".into()],
            }
        );
    }

    #[test]
    fn msg_escaped_quotes_in_strings() {
        assert_eq!(
            parse(r#"MSG:2010,0,1,"Optical drive \"BD-RE BU40N\" opened in OS access mode.","Optical drive \"%1\" opened in OS access mode.","BD-RE BU40N""#),
            Token::Message {
                code: 2010,
                flags: MsgFlags(0),
                count: 1,
                message: r#"Optical drive "BD-RE BU40N" opened in OS access mode."#.into(),
                format: r#"Optical drive "%1" opened in OS access mode."#.into(),
                params: vec!["BD-RE BU40N".into()],
            }
        );
    }

    #[test]
    fn msg_escaped_backslash() {
        assert_eq!(
            parse(r#"MSG:1234,0,1,"Path is C:\\foo","Path is %1","C:\\foo""#),
            Token::Message {
                code: 1234,
                flags: MsgFlags(0),
                count: 1,
                message: r#"Path is C:\foo"#.into(),
                format: "Path is %1".into(),
                params: vec![r#"C:\foo"#.into()],
            }
        );
    }

    #[test]
    fn msg_trailing_newline() {
        let token = parse_line("MSG:5011,0,0,\"ok\",\"ok\"\n").unwrap();
        assert!(matches!(token, Token::Message { code: 5011, .. }));
    }

    #[test]
    fn prgc_basic() {
        assert_eq!(
            parse(r#"PRGC:5055,0,"Saving to MKV file""#),
            Token::ProgressCurrentTitle { code: 5055, id: 0, name: "Saving to MKV file".into() }
        );
    }

    #[test]
    fn prgt_basic() {
        assert_eq!(
            parse(r#"PRGT:5056,1,"Saving title 1 of 6""#),
            Token::ProgressTotalTitle { code: 5056, id: 1, name: "Saving title 1 of 6".into() }
        );
    }

    #[test]
    fn prgv_mid_progress() {
        assert_eq!(
            parse("PRGV:1024,32768,65536"),
            Token::ProgressValues { current: 1024, total: 32768, max: 65536 }
        );
    }

    #[test]
    fn prgv_zero() {
        assert_eq!(
            parse("PRGV:0,0,65536"),
            Token::ProgressValues { current: 0, total: 0, max: 65536 }
        );
    }

    #[test]
    fn prgv_complete() {
        assert_eq!(
            parse("PRGV:65536,65536,65536"),
            Token::ProgressValues { current: 65536, total: 65536, max: 65536 }
        );
    }

    #[test]
    fn drv_visible_and_enabled() {
        assert_eq!(
            parse(r#"DRV:0,1,1,0,"BD-RE BW-16D1HT","MyDisc""#),
            Token::Drive {
                index: 0,
                visible: true,
                enabled: true,
                flags: DiskFlags(0),
                drive_name: "BD-RE BW-16D1HT".into(),
                disc_name: "MyDisc".into(),
            }
        );
    }

    #[test]
    fn drv_not_visible() {
        assert_eq!(
            parse(r#"DRV:1,0,0,0,"","" "#),
            Token::Drive {
                index: 1,
                visible: false,
                enabled: false,
                flags: DiskFlags(0),
                drive_name: "".into(),
                disc_name: "".into(),
            }
        );
    }

    #[test]
    fn drv_visible_not_enabled() {
        assert_eq!(
            parse(r#"DRV:2,1,0,4,"DRIVE NAME","" "#),
            Token::Drive {
                index: 2,
                visible: true,
                enabled: false,
                flags: DiskFlags(4),
                drive_name: "DRIVE NAME".into(),
                disc_name: "".into(),
            }
        );
    }

    #[test]
    fn tcount_basic() {
        assert_eq!(parse("TCOUNT:6"), Token::TitleCount { count: 6 });
    }

    #[test]
    fn tcount_zero() {
        assert_eq!(parse("TCOUNT:0"), Token::TitleCount { count: 0 });
    }

    #[test]
    fn cinfo_with_message_code() {
        assert_eq!(
            parse(r#"CINFO:1,6209,"Blu-ray disc""#),
            Token::DiscAttribute { id: AttributeId::Type, code: 6209, value: "Blu-ray disc".into() }
        );
    }

    #[test]
    fn cinfo_no_message_code() {
        assert_eq!(
            parse(r#"CINFO:2,0,"MyDisc""#),
            Token::DiscAttribute { id: AttributeId::Name, code: 0, value: "MyDisc".into() }
        );
    }

    #[test]
    fn cinfo_html_value() {
        assert_eq!(
            parse(r#"CINFO:31,6119,"<b>Source information</b><br>""#),
            Token::DiscAttribute { id: AttributeId::PanelTitle, code: 6119, value: "<b>Source information</b><br>".into() }
        );
    }

    #[test]
    fn tinfo_basic() {
        assert_eq!(
            parse(r#"TINFO:0,2,0,"MyDisc""#),
            Token::TitleAttribute { title_index: 0, id: AttributeId::Name, code: 0, value: "MyDisc".into() }
        );
    }

    #[test]
    fn tinfo_comma_in_value() {
        assert_eq!(
            parse(r#"TINFO:0,30,0,"MyDisc - 20 chapter(s) , 77.2 GB""#),
            Token::TitleAttribute {
                title_index: 0,
                id: AttributeId::TreeInfo,
                code: 0,
                value: "MyDisc - 20 chapter(s) , 77.2 GB".into(),
            }
        );
    }

    #[test]
    fn tinfo_multiple_commas_in_value() {
        assert_eq!(
            parse(r#"TINFO:1,26,0,"174,175,175,175,473""#),
            Token::TitleAttribute {
                title_index: 1,
                id: AttributeId::SegmentsMap,
                code: 0,
                value: "174,175,175,175,473".into(),
            }
        );
    }

    #[test]
    fn tinfo_html_value() {
        assert_eq!(
            parse(r#"TINFO:0,31,6120,"<b>Title information</b><br>""#),
            Token::TitleAttribute {
                title_index: 0,
                id: AttributeId::PanelTitle,
                code: 6120,
                value: "<b>Title information</b><br>".into(),
            }
        );
    }

    #[test]
    fn tinfo_duration_with_colons() {
        assert_eq!(
            parse(r#"TINFO:0,9,0,"2:05:20""#),
            Token::TitleAttribute { title_index: 0, id: AttributeId::Duration, code: 0, value: "2:05:20".into() }
        );
    }

    #[test]
    fn tinfo_nonzero_title_index() {
        assert_eq!(
            parse(r#"TINFO:5,27,0,"MyDisc_t05.mkv""#),
            Token::TitleAttribute { title_index: 5, id: AttributeId::OutputFileName, code: 0, value: "MyDisc_t05.mkv".into() }
        );
    }

    #[test]
    fn sinfo_basic() {
        assert_eq!(
            parse(r#"SINFO:0,0,1,6201,"Video""#),
            Token::StreamAttribute {
                title_index: 0,
                stream_index: 0,
                id: AttributeId::Type,
                code: 6201,
                value: "Video".into(),
            }
        );
    }

    #[test]
    fn sinfo_empty_value() {
        assert_eq!(
            parse(r#"SINFO:0,0,38,0,"""#),
            Token::StreamAttribute {
                title_index: 0,
                stream_index: 0,
                id: AttributeId::MkvFlags,
                code: 0,
                value: "".into(),
            }
        );
    }

    #[test]
    fn sinfo_complex_codec_string() {
        assert_eq!(
            parse(r#"SINFO:0,0,7,0,"MpegH HEVC Main10@L5.1 (dvhe.07.06 BL+FEL+RPU)""#),
            Token::StreamAttribute {
                title_index: 0,
                stream_index: 0,
                id: AttributeId::CodecLong,
                code: 0,
                value: "MpegH HEVC Main10@L5.1 (dvhe.07.06 BL+FEL+RPU)".into(),
            }
        );
    }

    #[test]
    fn sinfo_framerate_slash_and_parens() {
        assert_eq!(
            parse(r#"SINFO:0,0,21,0,"23.976 (480000/20020)""#),
            Token::StreamAttribute {
                title_index: 0,
                stream_index: 0,
                id: AttributeId::VideoFrameRate,
                code: 0,
                value: "23.976 (480000/20020)".into(),
            }
        );
    }

    #[test]
    fn sinfo_nonzero_stream_index() {
        assert_eq!(
            parse(r#"SINFO:0,1,2,0,"Surround 7.1""#),
            Token::StreamAttribute {
                title_index: 0,
                stream_index: 1,
                id: AttributeId::Name,
                code: 0,
                value: "Surround 7.1".into(),
            }
        );
    }

    #[test]
    fn sinfo_nonzero_title_and_stream() {
        assert_eq!(
            parse(r#"SINFO:2,3,30,0,"DD Surround 5.1 Spanish""#),
            Token::StreamAttribute {
                title_index: 2,
                stream_index: 3,
                id: AttributeId::TreeInfo,
                code: 0,
                value: "DD Surround 5.1 Spanish".into(),
            }
        );
    }

    #[test]
    fn sinfo_html_value() {
        assert_eq!(
            parse(r#"SINFO:0,0,31,6121,"<b>Track information</b><br>""#),
            Token::StreamAttribute {
                title_index: 0,
                stream_index: 0,
                id: AttributeId::PanelTitle,
                code: 6121,
                value: "<b>Track information</b><br>".into(),
            }
        );
    }

    #[test]
    fn attribute_id_all_sample_ids() {
        assert_eq!(AttributeId::from(1u32),  AttributeId::Type);
        assert_eq!(AttributeId::from(2u32),  AttributeId::Name);
        assert_eq!(AttributeId::from(3u32),  AttributeId::LangCode);
        assert_eq!(AttributeId::from(4u32),  AttributeId::LangName);
        assert_eq!(AttributeId::from(5u32),  AttributeId::CodecId);
        assert_eq!(AttributeId::from(6u32),  AttributeId::CodecShort);
        assert_eq!(AttributeId::from(7u32),  AttributeId::CodecLong);
        assert_eq!(AttributeId::from(8u32),  AttributeId::ChapterCount);
        assert_eq!(AttributeId::from(9u32),  AttributeId::Duration);
        assert_eq!(AttributeId::from(10u32), AttributeId::DiskSize);
        assert_eq!(AttributeId::from(11u32), AttributeId::DiskSizeBytes);
        assert_eq!(AttributeId::from(13u32), AttributeId::Bitrate);
        assert_eq!(AttributeId::from(14u32), AttributeId::AudioChannelsCount);
        assert_eq!(AttributeId::from(16u32), AttributeId::SourceFileName);
        assert_eq!(AttributeId::from(17u32), AttributeId::AudioSampleRate);
        assert_eq!(AttributeId::from(18u32), AttributeId::AudioSampleSize);
        assert_eq!(AttributeId::from(19u32), AttributeId::VideoSize);
        assert_eq!(AttributeId::from(20u32), AttributeId::VideoAspectRatio);
        assert_eq!(AttributeId::from(21u32), AttributeId::VideoFrameRate);
        assert_eq!(AttributeId::from(22u32), AttributeId::StreamFlags);
        assert_eq!(AttributeId::from(25u32), AttributeId::SegmentsCount);
        assert_eq!(AttributeId::from(26u32), AttributeId::SegmentsMap);
        assert_eq!(AttributeId::from(27u32), AttributeId::OutputFileName);
        assert_eq!(AttributeId::from(28u32), AttributeId::MetadataLanguageCode);
        assert_eq!(AttributeId::from(29u32), AttributeId::MetadataLanguageName);
        assert_eq!(AttributeId::from(30u32), AttributeId::TreeInfo);
        assert_eq!(AttributeId::from(31u32), AttributeId::PanelTitle);
        assert_eq!(AttributeId::from(32u32), AttributeId::VolumeName);
        assert_eq!(AttributeId::from(33u32), AttributeId::OrderWeight);
        assert_eq!(AttributeId::from(38u32), AttributeId::MkvFlags);
        assert_eq!(AttributeId::from(39u32), AttributeId::MkvFlagsText);
        assert_eq!(AttributeId::from(40u32), AttributeId::AudioChannelLayoutName);
        assert_eq!(AttributeId::from(42u32), AttributeId::OutputConversionType);
    }

    #[test]
    fn attribute_id_full_range() {
        assert_eq!(AttributeId::from(0u32),  AttributeId::Unknown);
        assert_eq!(AttributeId::from(12u32), AttributeId::StreamTypeExtension);
        assert_eq!(AttributeId::from(15u32), AttributeId::AngleInfo);
        assert_eq!(AttributeId::from(23u32), AttributeId::DateTime);
        assert_eq!(AttributeId::from(24u32), AttributeId::OriginalTitleId);
        assert_eq!(AttributeId::from(34u32), AttributeId::OutputFormat);
        assert_eq!(AttributeId::from(35u32), AttributeId::OutputFormatDescription);
        assert_eq!(AttributeId::from(36u32), AttributeId::SeamlessInfo);
        assert_eq!(AttributeId::from(37u32), AttributeId::PanelText);
        assert_eq!(AttributeId::from(41u32), AttributeId::OutputCodecShort);
        assert_eq!(AttributeId::from(43u32), AttributeId::OutputAudioSampleRate);
        assert_eq!(AttributeId::from(44u32), AttributeId::OutputAudioSampleSize);
        assert_eq!(AttributeId::from(45u32), AttributeId::OutputAudioChannelsCount);
        assert_eq!(AttributeId::from(46u32), AttributeId::OutputAudioChannelLayoutName);
        assert_eq!(AttributeId::from(47u32), AttributeId::OutputAudioChannelLayout);
        assert_eq!(AttributeId::from(48u32), AttributeId::OutputAudioMixDescription);
        assert_eq!(AttributeId::from(49u32), AttributeId::Comment);
        assert_eq!(AttributeId::from(50u32), AttributeId::OffsetSequenceId);
    }

    #[test]
    fn attribute_id_other() {
        assert_eq!(AttributeId::from(51u32),    AttributeId::Other(51));
        assert_eq!(AttributeId::from(255u32),   AttributeId::Other(255));
        assert_eq!(AttributeId::from(u32::MAX), AttributeId::Other(u32::MAX));
    }

    #[test]
    fn msgflags_none() {
        let f = MsgFlags(0);
        assert!(!f.is_debug());
        assert!(!f.is_hidden());
        assert!(!f.is_event());
        assert!(!f.requires_response());
        assert!(!f.is_error());
        assert!(!f.has_url());
    }

    #[test]
    fn msgflags_debug() {
        assert!(MsgFlags(32).is_debug());
        assert!(!MsgFlags(32).requires_response());
    }

    #[test]
    fn msgflags_hidden() {
        assert!(MsgFlags(64).is_hidden());
    }

    #[test]
    fn msgflags_event() {
        assert!(MsgFlags(128).is_event());
    }

    #[test]
    fn msgflags_boxok() {
        let f = MsgFlags(260);
        assert!(f.requires_response());
        assert!(!f.is_error());
    }

    #[test]
    fn msgflags_boxerror() {
        let f = MsgFlags(516);
        assert!(f.requires_response());
        assert!(f.is_error());
    }

    #[test]
    fn msgflags_boxwarning() {
        let f = MsgFlags(1028);
        assert!(f.requires_response());
        assert!(!f.is_error());
    }

    #[test]
    fn msgflags_boxyesno() {
        let f = MsgFlags(776);
        assert!(f.requires_response());
        assert!(!f.is_error());
    }

    #[test]
    fn msgflags_has_url() {
        assert!(MsgFlags(131072).has_url());
        assert!(!MsgFlags(0).has_url());
    }

    #[test]
    fn diskflags_none() {
        let f = DiskFlags(0);
        assert!(!f.has_dvd());
        assert!(!f.has_hdvd());
        assert!(!f.has_bluray());
        assert!(!f.has_aacs());
        assert!(!f.has_bdsvm());
    }

    #[test]
    fn diskflags_dvd_only() {
        let f = DiskFlags(1);
        assert!(f.has_dvd());
        assert!(!f.has_bluray());
        assert!(!f.has_aacs());
    }

    #[test]
    fn diskflags_bluray_with_aacs() {
        let f = DiskFlags(12);
        assert!(!f.has_dvd());
        assert!(f.has_bluray());
        assert!(f.has_aacs());
        assert!(!f.has_bdsvm());
    }

    #[test]
    fn diskflags_all() {
        let f = DiskFlags(31);
        assert!(f.has_dvd());
        assert!(f.has_hdvd());
        assert!(f.has_bluray());
        assert!(f.has_aacs());
        assert!(f.has_bdsvm());
    }

    #[test]
    fn error_unknown_kind() {
        match parse_line("UNKNOWN:foo").unwrap_err() {
            ParseError::UnknownKind(k) => assert_eq!(k, "UNKNOWN"),
            e => panic!("expected UnknownKind, got {e:?}"),
        }
    }

    #[test]
    fn error_no_colon() {
        assert!(matches!(parse_line("garbage").unwrap_err(), ParseError::NotAToken));
    }

    #[test]
    fn error_not_a_token_usage_line() {
        assert!(matches!(parse_line("Usage: makemkv [options]").unwrap_err(), ParseError::UnknownKind(_)));
    }

    #[test]
    fn error_invalid_integer() {
        assert!(matches!(
            parse_line("PRGV:abc,0,65536").unwrap_err(),
            ParseError::InvalidInt(_)
        ));
    }

    #[test]
    fn error_unterminated_string() {
        assert!(matches!(
            parse_line(r#"CINFO:1,0,"unterminated"#).unwrap_err(),
            ParseError::UnterminatedString
        ));
    }

    #[test]
    fn error_missing_string_field() {
        assert!(matches!(
            parse_line("CINFO:1,0").unwrap_err(),
            ParseError::MissingField
        ));
    }

    #[test]
    fn error_missing_integer_field() {
        assert!(matches!(
            parse_line("PRGV:0,0").unwrap_err(),
            ParseError::InvalidInt(_)
        ));
    }
}
