extern crate chrono;
extern crate combine;
use combine::error::{ParseError, StreamError};
use combine::{count_min_max, token, Parser, Stream};
use combine::parser::byte::digit;
use combine::stream::StreamErrorFor;
use chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};

fn date<'a, I>() -> impl Parser<Input = I, Output = NaiveDate> + 'a
where
    I: Stream<Item = u8, Range = &'a [u8]> + 'a,
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    (
        count_min_max::<Vec<u8>, _>(4, 4, digit()),
        token(b':'),
        count_min_max::<Vec<u8>, _>(2, 2, digit()),
        token(b':'),
        count_min_max::<Vec<u8>, _>(2, 2, digit()),
    ).and_then(|(y, _, m, _, d)| {
        let y: i32 = String::from_utf8(y).map_err(StreamErrorFor::<I>::other)?.parse().map_err(StreamErrorFor::<I>::other)?;
        let m: u32 = String::from_utf8(m).map_err(StreamErrorFor::<I>::other)?.parse().map_err(StreamErrorFor::<I>::other)?;
        let d: u32 = String::from_utf8(d).map_err(StreamErrorFor::<I>::other)?.parse().map_err(StreamErrorFor::<I>::other)?;
        NaiveDate::from_ymd_opt(y, m, d).ok_or(StreamErrorFor::<I>::unexpected_static_message(
            "failed to build NaiveDate",
        ))
    })
}

fn time<'a, I>() -> impl Parser<Input = I, Output = NaiveTime> + 'a
where
    I: Stream<Item = u8, Range = &'a [u8]> + 'a,
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    (
        count_min_max::<Vec<u8>, _>(2, 2, digit()),
        token(b':'),
        count_min_max::<Vec<u8>, _>(2, 2, digit()),
        token(b':'),
        count_min_max::<Vec<u8>, _>(2, 2, digit()),
    ).and_then(|(h, _, m, _, s)| {
        let h: u32 = String::from_utf8(h).map_err(StreamErrorFor::<I>::other)?.parse().map_err(StreamErrorFor::<I>::other)?;
        let m: u32 = String::from_utf8(m).map_err(StreamErrorFor::<I>::other)?.parse().map_err(StreamErrorFor::<I>::other)?;
        let s: u32 = String::from_utf8(s).map_err(StreamErrorFor::<I>::other)?.parse().map_err(StreamErrorFor::<I>::other)?;
        NaiveTime::from_hms_opt(h, m, s).ok_or(StreamErrorFor::<I>::unexpected_static_message(
            "failed to build NaiveTime",
        ))
    })
}

fn datetime<'a, I>() -> impl Parser<Input = I, Output = NaiveDateTime> + 'a
where
    I: Stream<Item = u8, Range = &'a [u8]> + 'a,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    (date(), token(b'-'), time()).map(|(date, _, time)| NaiveDateTime::new(date, time))
}

#[cfg(test)]
mod tests {
    use combine::Parser;
    use chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};

    #[test]
    fn date() {
        assert_eq!(
            ::date().parse(&b"2016:04:03"[..]),
            Ok((NaiveDate::from_ymd(2016, 4, 3), &b""[..]))
        );
    }

    #[test]
    fn time() {
        assert_eq!(
            super::time().parse(&b"23:59:59"[..]),
            Ok((NaiveTime::from_hms(23, 59, 59), &b""[..]))
        );
    }

    #[test]
    fn datetime() {
        let want = NaiveDateTime::new(
            NaiveDate::from_ymd(2016, 4, 3),
            NaiveTime::from_hms(23, 59, 59),
        );
        assert_eq!(
            super::datetime().parse(&b"2016:04:03-23:59:59"[..]),
            Ok((want, &b""[..]))
        );
    }
}

pub mod dhcp {
    use chrono::NaiveDateTime;
    use combine::{
        many1, satisfy, token, Parser, Stream, try, choice, count_min_max,
        error::{ParseError, StreamError},
        stream::StreamErrorFor,
        parser::byte::{digit, space, bytes}};

    #[derive(Debug, PartialEq, Clone)]
    pub struct LogEntry {
        pub datetime: NaiveDateTime,
        pub msg: DhcpMsg,
    }

    #[derive(Debug, PartialEq, Clone)]
    pub enum DhcpMsg {
        Inform,
        Offer,
        Ack{ ip_addr: String, mac_addr: String },
        Nak,
        Request,
        Discover,
    }

    impl LogEntry {
        pub fn new(s: &[u8]) -> Result<Self, Box<::std::error::Error>> {
            log_entry().easy_parse(s).map(|x| x.0).map_err(|_| "error".into())
        }
    }

    fn log_entry<'a, I>() -> impl Parser<Input = I, Output = LogEntry> + 'a
    where
        I: Stream<Item = u8, Range = &'a [u8]> + 'a,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            ::datetime(),
            many1::<Vec<u8>, _>(satisfy(|c| c != b':')),
            token(b':'),
            space(),
            dhcp_msg(),
        ).map(|(datetime, _, _, _, msg)| LogEntry { datetime, msg })
    }

    fn dhcp_msg<'a, I>() -> impl Parser<Input = I, Output = DhcpMsg> + 'a
    where
        I: Stream<Item = u8, Range = &'a [u8]> + 'a,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            bytes(&b"DHCP"[..]),
            choice((
                try(bytes(&b"INFORM"[..]).map(|_| DhcpMsg::Inform)),
                try(bytes(&b"OFFER"[..]).map(|_| DhcpMsg::Offer)),
                try(bytes(&b"ACK"[..]).with(dhcp_ack())),
                try(bytes(&b"NAK"[..]).map(|_| DhcpMsg::Nak)),
                try(bytes(&b"REQUEST"[..]).map(|_| DhcpMsg::Request)),
                try(bytes(&b"DISCOVER"[..]).map(|_| DhcpMsg::Discover)),
            ))
        ).map(|(_, msg)| msg)
    }

    fn dhcp_ack<'a, I>() -> impl Parser<Input = I, Output = DhcpMsg> + 'a
    where
        I: Stream<Item = u8, Range = &'a [u8]> + 'a,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            space(),
            bytes(&b"on "[..]).or(bytes(&b"to "[..])),
            ip_addr(),
            space(),
            bytes(&b"to "[..]).or(bytes(&b"("[..])),
            mac_addr(),
        ).map(|(_, _, ip_addr, _, _, mac_addr)| {
            DhcpMsg::Ack{ip_addr, mac_addr}
        })
    }

    fn ip_addr<'a, I>() -> impl Parser<Input = I, Output = String> + 'a
    where
        I: Stream<Item = u8, Range = &'a [u8]> + 'a,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            count_min_max::<Vec<u8>, _>(1, 3, digit()),
            token(b'.'),
            count_min_max::<Vec<u8>, _>(1, 3, digit()),
            token(b'.'),
            count_min_max::<Vec<u8>, _>(1, 3, digit()),
            token(b'.'),
            count_min_max::<Vec<u8>, _>(1, 3, digit()),
        ).map(|(a, _, b, _, c, _, d)| {
            let a = String::from_utf8(a).unwrap();
            let b = String::from_utf8(b).unwrap();
            let c = String::from_utf8(c).unwrap();
            let d = String::from_utf8(d).unwrap();
            format!("{}.{}.{}.{}", a, b, c, d)
        })
    }

    fn mac_addr<'a, I>() -> impl Parser<Input = I, Output = String> + 'a
    where
        I: Stream<Item = u8, Range = &'a [u8]> + 'a,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        count_min_max::<Vec<u8>, _>(17, 17, satisfy(|c: u8| c.is_ascii_hexdigit() || c == b':')).and_then(|b| String::from_utf8(b).map_err(StreamErrorFor::<I>::other))
    }

    #[cfg(test)]
    mod tests {
        use super::{LogEntry, DhcpMsg};
        use combine::Parser;
        use chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};

        #[test]
        fn mac_addr() {
            assert_eq!(
                super::mac_addr().parse(&b"f4:ec:38:85:d8:a9"[..]),
                Ok(("f4:ec:38:85:d8:a9".to_string(),  &b""[..]))
            );
        }

        #[test]
        fn dhcp_ack() {
            assert_eq!(
                super::dhcp_ack().parse(&b" on 192.168.0.254 to a4:db:30:66:4f:90"[..]),
                Ok((DhcpMsg::Ack{ip_addr: "192.168.0.254".to_string(), mac_addr: "a4:db:30:66:4f:90".to_string()}, &b""[..]))
            );
            assert_eq!(
                super::dhcp_ack().parse(&b" to 192.168.0.77 (9c:ad:97:d1:65:39"[..]),
                Ok((DhcpMsg::Ack{ip_addr: "192.168.0.77".to_string(), mac_addr: "9c:ad:97:d1:65:39".to_string()}, &b""[..]))
            );
        }

        #[test]
        fn dhcp_msg() {
            assert_eq!(
                super::dhcp_msg().parse(&b"DHCPINFORM"[..]),
                Ok((DhcpMsg::Inform, &b""[..]))
            );
            assert_eq!(
                super::dhcp_msg().parse(&b"DHCPOFFER"[..]),
                Ok((DhcpMsg::Offer, &b""[..]))
            );
            assert_eq!(
                super::dhcp_msg().parse(&b"DHCPACK on 192.168.0.254 to a4:db:30:66:4f:90"[..]),
                Ok((DhcpMsg::Ack{ip_addr: "192.168.0.254".to_string(), mac_addr: "a4:db:30:66:4f:90".to_string()}, &b""[..]))
            );
            assert_eq!(
                super::dhcp_msg().parse(&b"DHCPACK to 192.168.0.77 (9c:ad:97:d1:65:39"[..]),
                Ok((DhcpMsg::Ack{ip_addr: "192.168.0.77".to_string(), mac_addr: "9c:ad:97:d1:65:39".to_string()}, &b""[..]))
            );
            assert_eq!(
                super::dhcp_msg().parse(&b"DHCPNAK"[..]),
                Ok((DhcpMsg::Nak, &b""[..]))
            );
            assert_eq!(
                super::dhcp_msg().parse(&b"DHCPREQUEST"[..]),
                Ok((DhcpMsg::Request, &b""[..]))
            );
            assert_eq!(
                super::dhcp_msg().parse(&b"DHCPDISCOVER"[..]),
                Ok((DhcpMsg::Discover, &b""[..]))
            );
        }

        #[test]
        fn log_entry() {
            let log = &br#"2015:06:03-00:01:00 PublicWiFi dhcpd: DHCPACK to 192.168.0.77 (9c:ad:97:d1:65:39"#[..];
            let want = LogEntry {
                datetime: NaiveDateTime::new(
                    NaiveDate::from_ymd(2015, 6, 3),
                    NaiveTime::from_hms(0, 1, 0),
                ),
                msg: DhcpMsg::Ack{ip_addr: "192.168.0.77".to_string(), mac_addr: "9c:ad:97:d1:65:39".to_string()}
            };
            assert_eq!(super::log_entry().parse(log), Ok((want, &b""[..])));

        }
    }
}

pub mod http {
    use combine::{
        between, eof, many, many1, satisfy, token, Parser, Stream,
        error::ParseError,
        parser::byte::{newline, space}};
    use std::collections::HashMap;
    use chrono::NaiveDateTime;

    #[derive(Debug, PartialEq, Clone)]
    pub struct LogEntry {
        pub datetime: NaiveDateTime,
        pub attrs: HashMap<String, Vec<u8>>,
    }

    impl LogEntry {
        pub fn new(s: &[u8]) -> Result<Self, Box<::std::error::Error>> {
            log_entry()
                .easy_parse(s)
                .map(|x| x.0)
                .map_err(|_| "error".into())
        }
    }

    fn log_entry<'a, I>() -> impl Parser<Input = I, Output = LogEntry> + 'a
    where
        I: Stream<Item = u8, Range = &'a [u8]> + 'a,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            ::datetime(),
            many1::<Vec<u8>, _>(satisfy(|c| c != b':')),
            token(b':'),
            space(),
            attrs(),
        ).map(|(datetime, _, _, _, attrs)| LogEntry { datetime, attrs })
    }

    fn attr<'a, I>() -> impl Parser<Input = I, Output = (String, Vec<u8>)> + 'a
    where
        I: Stream<Item = u8, Range = &'a [u8]> + 'a,
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            many1::<Vec<u8>, _>(satisfy(|c| c != b'=')),
            token(b'='),
            between(token(b'"'), token(b'"'), many(satisfy(|c| c != b'"'))),
        ).map(|(k, _, v)| {
            let k = String::from_utf8(k).unwrap();
            (k, v)
        })
    }

    fn attrs<'a, I>() -> impl Parser<Input = I, Output = HashMap<String, Vec<u8>>> + 'a
    where
        I: Stream<Item = u8, Range = &'a [u8]> + 'a,
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        let eol = || newline().or(eof().map(|_| b' '));
        many(attr().skip(space().or(eol())))
    }

    #[cfg(test)]
    mod tests {
        use super::LogEntry;
        use combine::Parser;
        use std::collections::HashMap;
        use chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};
        #[test]
        fn attr() {
            assert_eq!(
                super::attr().parse(&b"foo=\"bar\""[..]),
                Ok((("foo".to_string(), b"bar"[..].to_vec()), &b""[..]))
            );
        }

        #[test]
        fn attrs() {
            let want: HashMap<String, Vec<u8>> = [
                ("foo".to_string(), b"bar"[..].to_vec()),
                ("bat".to_string(), b"baz"[..].to_vec()),
            ].iter()
                .cloned()
                .collect();
            assert_eq!(
                super::attrs().parse(&b"foo=\"bar\" bat=\"baz\""[..]),
                Ok((want, &b""[..]))
            );
        }

        #[test]
        fn date() {
            assert_eq!(
                ::date().parse(&b"2016:04:03"[..]),
                Ok((NaiveDate::from_ymd(2016, 4, 3), &b""[..]))
            );
        }

        #[test]
        fn time() {
            assert_eq!(
                ::time().parse(&b"23:59:59"[..]),
                Ok((NaiveTime::from_hms(23, 59, 59), &b""[..]))
            );
        }

        #[test]
        fn datetime() {
            let want = NaiveDateTime::new(
                NaiveDate::from_ymd(2016, 4, 3),
                NaiveTime::from_hms(23, 59, 59),
            );
            assert_eq!(
                ::datetime().parse(&b"2016:04:03-23:59:59"[..]),
                Ok((want, &b""[..]))
            );
        }

        #[test]
        fn log_entry() {
            let log = &br#"2016:04:03-23:59:59 publicwifi httpproxy[18500]: foo="bar" bat="baz""#[..];
            let want = LogEntry {
                datetime: NaiveDateTime::new(
                    NaiveDate::from_ymd(2016, 4, 3),
                    NaiveTime::from_hms(23, 59, 59),
                ),
                attrs: [
                    ("foo".to_string(), b"bar"[..].to_vec()),
                    ("bat".to_string(), b"baz"[..].to_vec()),
                ].iter()
                    .cloned()
                    .collect(),
            };
            assert_eq!(super::log_entry().parse(log), Ok((want.clone(), &b""[..])));
            let logn = &b"2016:04:03-23:59:59 publicwifi httpproxy[18500]: foo=\"bar\" bat=\"baz\"\n"[..];
            assert_eq!(super::log_entry().parse(logn), Ok((want.clone(), &b""[..])));
        }
    }

}
