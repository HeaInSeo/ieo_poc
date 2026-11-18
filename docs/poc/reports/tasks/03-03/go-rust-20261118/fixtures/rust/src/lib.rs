pub fn double(value: i32) -> i32 {
    2 * value
}

#[cfg(test)]
mod tests {
    #[test]
    fn doubles_three() {
        assert_eq!(super::double(3), 6);
    }
}
