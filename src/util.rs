
#[derive(PartialEq, Debug)]
pub enum ContainsResult {
    DoesNotContain,
    PossiblyContains (usize),
    Contains (usize)
}

/// Returns option of position of one subslice in an other
pub fn contains(slice: &[u8], subslice: &[u8]) -> ContainsResult {

    let mut streak = 0;
    let mut i = 0;
    while i < slice.len() {
        if slice[i] == subslice[streak] {
            streak += 1;
            if streak == subslice.len() {
                return ContainsResult::Contains (i + 1 - streak);
            }
        } else {
            if streak > 0 {
                i -= streak;
                streak = 0;
            }
        }
        i += 1;
    }
    if streak > 0 {
        return ContainsResult::PossiblyContains (slice.len() - streak);
    } else {
        return ContainsResult::DoesNotContain;
    }
}





pub mod multipart {

    #[derive(Debug)]
    pub(crate) struct BodyPartInfo {
        pub content_type: Option<String>,
        pub field_name: String,
        pub file_name: Option<String>
    }

    impl BodyPartInfo {
        pub fn from_headers(headers: &str) -> Result<Self, tide::Error> {
            let mut content_type = None;
            let mut file_name = None;
            let mut field_name = None;

            for header in headers.split("\r\n") {
                if header.is_empty() { continue; }
                let mut header_split = header.split(": ");
                match header_split.next().ok_or(tide::Error::from_str(400, "Malformed body"))? {
                    "Content-Disposition" => {
                        let mut disposition_split = header_split.next()
                                .ok_or(tide::Error::from_str(400, "Malformed body"))?
                                .split(";");
                        let content_disposition = disposition_split.next()
                            .ok_or(tide::Error::from_str(400, "Malformed body"))?;
                        if content_disposition.trim() != "form-data" {
                            return Err(tide::Error::from_str(400, "Unknown content disposition"));
                        }
                        for other in disposition_split.into_iter() {
                            let mut key_value = other.trim().split("=");
                            let key = key_value.next()
                                .ok_or(tide::Error::from_str(400, "Malformed body"))?;
                            let value = key_value.next()
                                .ok_or(tide::Error::from_str(400, "Malformed body"))?
                                .trim_matches('"');
                        
                            match key { 
                                "name" => { field_name = Some(value.to_owned()) },
                                "filename" => { file_name = Some(value.to_owned()) },
                                _ => {}
                            }


                        }
                    },
                    "Content-Type" => {
                        content_type = header_split.next().map(|v| v.to_owned()) ;
                    },
                    _ => {}

                }

            };
            let field_name = field_name
                .ok_or(tide::Error::from_str(400, "No fielname provided"))?;
            return Ok(Self { field_name, content_type, file_name });
        }
    }
}








#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contains_works_with_bytes() {
        assert_eq!(contains(&[1,2,3,4,5], &[1,2,3]), ContainsResult::Contains(0));
        assert_eq!(contains(&[1,2,3,4,5], &[3,4,5]), ContainsResult::Contains(2));
        assert_eq!(contains(&[1,2,1,2,3], &[1,2,1]), ContainsResult::Contains(0));
        assert_eq!(contains(&[1,2,1,2,3], &[1,2,3]), ContainsResult::Contains(2));
        assert_eq!(contains(&[1,2,3,4,5], &[1,2,5]), ContainsResult::DoesNotContain);
        assert_eq!(contains(&[1,2,1,2,3], &[1,2,4]), ContainsResult::DoesNotContain);
        assert_eq!(contains(&[1,2,3,4,5], &[4,5,6]), ContainsResult::PossiblyContains(3));

    }

    #[test]
    fn contains_works_with_strs() {
        assert_eq!(contains(b"hello world", b"hello"), ContainsResult::Contains(0));
        assert_eq!(contains(b"hello there", b"there"), ContainsResult::Contains(6));
        assert_eq!(contains(b"----hello", b"--hello"), ContainsResult::Contains(2));
        assert_eq!(contains(b"----hell", b"--hello"), ContainsResult::PossiblyContains(2));
        assert_eq!(contains(b"--hello there general", b"hello there"), ContainsResult::Contains(2));
        assert_eq!(contains(b"hello there general", b"general kenobi"), ContainsResult::PossiblyContains(12));
        assert_eq!(contains(b"hello there kenobi", b"general kenobi"), ContainsResult::DoesNotContain);
    }

    #[test]
    fn contains_works_with_reqs() {
        assert_eq!(
            contains(
                b"------WebKitFormBoundaryGkEAO60J3WyaOnEr\r\nContent-Disposition: form-data; name=\"t", 
                b"----WebKitFormBoundaryGkEAO60J3WyaOnEr"
            ), 
            ContainsResult::Contains(2)
        );
        assert_eq!(
            contains(
                b"------WebKitFormBoundaryGkEAO60J3Wya", 
                b"----WebKitFormBoundaryGkEAO60J3WyaOnEr"
            ), 
            ContainsResult::PossiblyContains(2)
        );
    }


}



