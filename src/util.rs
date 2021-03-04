/// Returns option of position of one subslice in an other
pub fn contains(slice: &[u8], subslice: &[u8]) -> Option<usize> {
    if slice.len() < subslice.len() { return None; }

    let mut streak = 0;
    let mut i = 0;
    while i < slice.len() {
        if slice[i] == subslice[streak] {
            streak += 1;
            if streak == subslice.len() {
                return Some(i + 1 - streak);
            }
        } else {
            if streak > 0 {
                i -= streak;
                streak = 0;
            }
        }
        if slice.len() + streak < subslice.len() + i {
            //break;
        }
        i += 1;
    }
    return None;
}


#[cfg(test)]
mod tests {
    #[test]
    fn contains_works_with_bytes() {
        assert_eq!(super::contains(&[1,2,3,4,5], &[1,2,3]), Some(0));
        assert_eq!(super::contains(&[1,2,3,4,5], &[3,4,5]), Some(2));
        assert_eq!(super::contains(&[1,2,1,2,3], &[1,2,1]), Some(0));
        assert_eq!(super::contains(&[1,2,1,2,3], &[1,2,3]), Some(2));
        assert_eq!(super::contains(&[1,2,3,4,5], &[1,2,5]), None);
        assert_eq!(super::contains(&[1,2,3,4,5], &[4,5,6]), None);
        assert_eq!(super::contains(&[1,2,1,2,3], &[1,2,4]), None);

    }

    #[test]
    fn contains_works_with_strs() {
        assert_eq!(super::contains(b"hello world", b"hello"), Some(0));
        assert_eq!(super::contains(b"hello there", b"there"), Some(6));
        assert_eq!(super::contains(b"----hello", b"--hello"), Some(2));
        assert_eq!(super::contains(b"--hello there general", b"hello there"), Some(2));
        assert_eq!(super::contains(b"hello there general", b"general kenobi"), None);
    }

    #[test]
    fn contains_works_with_reqs() {
        assert_eq!(super::contains(
            b"------WebKitFormBoundaryGkEAO60J3WyaOnEr\r\nContent-Disposition: form-data; name=\"t", 
              b"----WebKitFormBoundaryGkEAO60J3WyaOnEr"), 
            Some(2)
        );
    }


}



