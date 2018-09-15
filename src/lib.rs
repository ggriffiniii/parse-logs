extern crate chrono;
extern crate combine;
use combine::error::{ParseError, StreamError};
use combine::{count_min_max, token, Parser, Stream};
use combine::parser::char::digit;
use combine::stream::StreamErrorFor;
use chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};

fn date<I>() -> impl Parser<Input = I, Output = NaiveDate>
where
    I: Stream<Item = char>,
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    (
        count_min_max::<String, _>(4, 4, digit()),
        token(':'),
        count_min_max::<String, _>(2, 2, digit()),
        token(':'),
        count_min_max::<String, _>(2, 2, digit()),
    ).and_then(|(y, _, m, _, d)| {
        let y: i32 = y.parse().map_err(StreamErrorFor::<I>::other)?;
        let m: u32 = m.parse().map_err(StreamErrorFor::<I>::other)?;
        let d: u32 = d.parse().map_err(StreamErrorFor::<I>::other)?;
        NaiveDate::from_ymd_opt(y, m, d).ok_or(StreamErrorFor::<I>::unexpected_static_message(
            "failed to build NaiveDate",
        ))
    })
}

fn time<I>() -> impl Parser<Input = I, Output = NaiveTime>
where
    I: Stream<Item = char>,
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    (
        count_min_max::<String, _>(2, 2, digit()),
        token(':'),
        count_min_max::<String, _>(2, 2, digit()),
        token(':'),
        count_min_max::<String, _>(2, 2, digit()),
    ).and_then(|(h, _, m, _, s)| {
        let h: u32 = h.parse().map_err(StreamErrorFor::<I>::other)?;
        let m: u32 = m.parse().map_err(StreamErrorFor::<I>::other)?;
        let s: u32 = s.parse().map_err(StreamErrorFor::<I>::other)?;
        NaiveTime::from_hms_opt(h, m, s).ok_or(StreamErrorFor::<I>::unexpected_static_message(
            "failed to build NaiveTime",
        ))
    })
}

fn datetime<I>() -> impl Parser<Input = I, Output = NaiveDateTime>
where
    I: Stream<Item = char>,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    (date(), token('-'), time()).map(|(date, _, time)| NaiveDateTime::new(date, time))
}

#[cfg(test)]
mod tests {
    use combine::Parser;
    use chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};

    #[test]
    fn date() {
        assert_eq!(
            crate::date().parse("2016:04:03"),
            Ok((NaiveDate::from_ymd(2016, 4, 3), ""))
        );
    }

    #[test]
    fn time() {
        assert_eq!(
            crate::time().parse("23:59:59"),
            Ok((NaiveTime::from_hms(23, 59, 59), ""))
        );
    }

    #[test]
    fn datetime() {
        let want = NaiveDateTime::new(
            NaiveDate::from_ymd(2016, 4, 3),
            NaiveTime::from_hms(23, 59, 59),
        );
        assert_eq!(
            crate::datetime().parse("2016:04:03-23:59:59"),
            Ok((want, ""))
        );
    }
}

pub mod dhcp {
    use chrono::NaiveDateTime;
    use combine::parser::char::digit;
    use combine::{
        many1, satisfy, token, Parser, Stream, try, choice, count_min_max,
        error::ParseError,
        parser::char::{space, string}};

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
        pub fn new(s: &str) -> Result<Self, Box<::std::error::Error>> {
            log_entry().easy_parse(s).map(|x| x.0).map_err(|e| format!("{}", e).into())
        }
    }

    fn log_entry<I>() -> impl Parser<Input = I, Output = LogEntry>
    where
        I: Stream<Item = char>,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            super::datetime(),
            many1::<String, _>(satisfy(|c| c != ':')),
            token(':'),
            space(),
            dhcp_msg(),
        ).map(|(datetime, _, _, _, msg)| LogEntry { datetime, msg })
    }

    fn dhcp_msg<I>() -> impl Parser<Input = I, Output = DhcpMsg>
    where
        I: Stream<Item = char>,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            string("DHCP"),
            choice((
                try(string("INFORM").map(|_| DhcpMsg::Inform)),
                try(string("OFFER").map(|_| DhcpMsg::Offer)),
                try(string("ACK").with(dhcp_ack())),
                try(string("NAK").map(|_| DhcpMsg::Nak)),
                try(string("REQUEST").map(|_| DhcpMsg::Request)),
                try(string("DISCOVER").map(|_| DhcpMsg::Discover)),
            ))
        ).map(|(_, msg)| msg)
    }

    fn dhcp_ack<I>() -> impl Parser<Input = I, Output = DhcpMsg>
    where
        I: Stream<Item = char>,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            space(),
            string("on ").or(string("to ")),
            ip_addr(),
            space(),
            string("to ").or(string("(")),
            mac_addr(),
        ).map(|(_, _, ip_addr, _, _, mac_addr)| DhcpMsg::Ack{ip_addr, mac_addr})
    }

    fn ip_addr<I>() -> impl Parser<Input = I, Output = String>
    where
        I: Stream<Item = char>,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            count_min_max::<String, _>(1, 3, digit()),
            token('.'),
            count_min_max::<String, _>(1, 3, digit()),
            token('.'),
            count_min_max::<String, _>(1, 3, digit()),
            token('.'),
            count_min_max::<String, _>(1, 3, digit()),
        ).map(|(a, _, b, _, c, _, d)| [a,b,c,d].join("."))
    }

    fn mac_addr<I>() -> impl Parser<Input = I, Output = String>
    where
        I: Stream<Item = char>,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        count_min_max::<String, _>(17, 17, satisfy(|c: char| c.is_ascii_hexdigit() || c == ':'))
    }

    #[cfg(test)]
    mod tests {
        use super::{LogEntry, DhcpMsg};
        use combine::Parser;
        use chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};

        #[test]
        fn mac_addr() {
            assert_eq!(
                super::mac_addr().parse("f4:ec:38:85:d8:a9"),
                Ok(("f4:ec:38:85:d8:a9".to_string(),  ""))
            );
        }

        #[test]
        fn dhcp_ack() {
            assert_eq!(
                super::dhcp_ack().parse(" on 192.168.0.254 to a4:db:30:66:4f:90"),
                Ok((DhcpMsg::Ack{ip_addr: "192.168.0.254".to_string(), mac_addr: "a4:db:30:66:4f:90".to_string()}, ""))
            );
            assert_eq!(
                super::dhcp_ack().parse(" to 192.168.0.77 (9c:ad:97:d1:65:39"),
                Ok((DhcpMsg::Ack{ip_addr: "192.168.0.77".to_string(), mac_addr: "9c:ad:97:d1:65:39".to_string()}, ""))
            );
        }

        #[test]
        fn dhcp_msg() {
            assert_eq!(
                super::dhcp_msg().parse("DHCPINFORM"),
                Ok((DhcpMsg::Inform, ""))
            );
            assert_eq!(
                super::dhcp_msg().parse("DHCPOFFER"),
                Ok((DhcpMsg::Offer, ""))
            );
            assert_eq!(
                super::dhcp_msg().parse("DHCPACK on 192.168.0.254 to a4:db:30:66:4f:90"),
                Ok((DhcpMsg::Ack{ip_addr: "192.168.0.254".to_string(), mac_addr: "a4:db:30:66:4f:90".to_string()}, ""))
            );
            assert_eq!(
                super::dhcp_msg().parse("DHCPACK to 192.168.0.77 (9c:ad:97:d1:65:39"),
                Ok((DhcpMsg::Ack{ip_addr: "192.168.0.77".to_string(), mac_addr: "9c:ad:97:d1:65:39".to_string()}, ""))
            );
            assert_eq!(
                super::dhcp_msg().parse("DHCPNAK"),
                Ok((DhcpMsg::Nak, ""))
            );
            assert_eq!(
                super::dhcp_msg().parse("DHCPREQUEST"),
                Ok((DhcpMsg::Request, ""))
            );
            assert_eq!(
                super::dhcp_msg().parse("DHCPDISCOVER"),
                Ok((DhcpMsg::Discover, ""))
            );
        }

        #[test]
        fn log_entry() {
            let log = r#"2015:06:03-00:01:00 PublicWiFi dhcpd: DHCPACK to 192.168.0.77 (9c:ad:97:d1:65:39"#;
            let want = LogEntry {
                datetime: NaiveDateTime::new(
                    NaiveDate::from_ymd(2015, 6, 3),
                    NaiveTime::from_hms(0, 1, 0),
                ),
                msg: DhcpMsg::Ack{ip_addr: "192.168.0.77".to_string(), mac_addr: "9c:ad:97:d1:65:39".to_string()}
            };
            assert_eq!(super::log_entry().parse(log), Ok((want, "")));

        }
    }
}

pub mod http {
    use combine::{
        between, eof, many, many1, satisfy, token, Parser, Stream,
        error::ParseError,
        parser::char::{newline, space}};
    use std::collections::HashMap;
    use chrono::NaiveDateTime;

    #[derive(Debug, PartialEq, Clone)]
    pub struct LogEntry {
        pub datetime: NaiveDateTime,
        pub attrs: HashMap<String, String>,
    }

    impl LogEntry {
        pub fn new(s: &str) -> Result<Self, Box<::std::error::Error>> {
            log_entry()
                .easy_parse(s)
                .map(|x| x.0)
                .map_err(|e| format!("{}", e).into())
        }
    }

    fn log_entry<I>() -> impl Parser<Input = I, Output = LogEntry>
    where
        I: Stream<Item = char>,
        // Necessary due to rust-lang/rust#24159
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            super::datetime(),
            many1::<String, _>(satisfy(|c| c != ':')),
            token(':'),
            space(),
            attrs(),
        ).map(|(datetime, _, _, _, attrs)| LogEntry { datetime, attrs })
    }

    fn attr<I>() -> impl Parser<Input = I, Output = (String, String)>
    where
        I: Stream<Item = char>,
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        (
            many1(satisfy(|c| c != '=')),
            token('='),
            between(token('"'), token('"'), many(satisfy(|c| c != '"'))),
        ).map(|(k, _, v)| (k, v))
    }

    fn attrs<I>() -> impl Parser<Input = I, Output = HashMap<String, String>>
    where
        I: Stream<Item = char>,
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        let eol = || newline().or(eof().map(|_| ' '));
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
                super::attr().parse("foo=\"bar\""),
                Ok((("foo".to_string(), "bar".to_string()), ""))
            );
        }

        #[test]
        fn attrs() {
            let want: HashMap<String, String> = [
                ("foo".to_string(), "bar".to_string()),
                ("bat".to_string(), "baz".to_string()),
            ].iter()
                .cloned()
                .collect();
            assert_eq!(
                super::attrs().parse("foo=\"bar\" bat=\"baz\""),
                Ok((want, ""))
            );
        }

        #[test]
        fn date() {
            assert_eq!(
                crate::date().parse("2016:04:03"),
                Ok((NaiveDate::from_ymd(2016, 4, 3), ""))
            );
        }

        #[test]
        fn time() {
            assert_eq!(
                crate::time().parse("23:59:59"),
                Ok((NaiveTime::from_hms(23, 59, 59), ""))
            );
        }

        #[test]
        fn datetime() {
            let want = NaiveDateTime::new(
                NaiveDate::from_ymd(2016, 4, 3),
                NaiveTime::from_hms(23, 59, 59),
            );
            assert_eq!(
                crate::datetime().parse("2016:04:03-23:59:59"),
                Ok((want, ""))
            );
        }

        #[test]
        fn log_entry() {
            let log = r#"2016:04:03-23:59:59 publicwifi httpproxy[18500]: foo="bar" bat="baz""#;
            let want = LogEntry {
                datetime: NaiveDateTime::new(
                    NaiveDate::from_ymd(2016, 4, 3),
                    NaiveTime::from_hms(23, 59, 59),
                ),
                attrs: [
                    ("foo".to_string(), "bar".to_string()),
                    ("bat".to_string(), "baz".to_string()),
                ].iter()
                    .cloned()
                    .collect(),
            };
            assert_eq!(super::log_entry().parse(log), Ok((want.clone(), "")));
            let logn = "2016:04:03-23:59:59 publicwifi httpproxy[18500]: foo=\"bar\" bat=\"baz\"\n";
            assert_eq!(super::log_entry().parse(logn), Ok((want.clone(), "")));
        }
    }

}
