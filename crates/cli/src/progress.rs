const DEFAULT_TERMINAL_WIDTH: usize = 80;
const MIN_TERMINAL_WIDTH: usize = 40;

pub fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width >= MIN_TERMINAL_WIDTH)
        .unwrap_or(DEFAULT_TERMINAL_WIDTH)
}

pub fn short_image_name(image: &str) -> String {
    let image = crate::docker::short_image_name(image);
    let Some((repository, tag)) = image.rsplit_once(':') else {
        return image.to_string();
    };
    if tag.len() > 12 && tag.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        format!("{repository}:{}", &tag[..12])
    } else {
        image.to_string()
    }
}

pub fn fit_line(line: &str, width: usize) -> String {
    let width = width.max(1);
    let count = line.chars().count();
    if count <= width {
        return line.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }
    let mut fitted = line.chars().take(width - 1).collect::<String>();
    fitted.push('…');
    fitted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immutable_image_revision_is_shortened() {
        assert_eq!(
            short_image_name(
                "ghcr.io/example/argus-control-api:1d5873f468f5d30b7bb6c1637a6817278983e207"
            ),
            "argus-control-api:1d5873f468f5"
        );
        assert_eq!(
            short_image_name("postgres:16-bookworm"),
            "postgres:16-bookworm"
        );
    }

    #[test]
    fn fitted_progress_never_exceeds_the_terminal_width() {
        let fitted = fit_line("a progress line which is much too long", 20);
        assert_eq!(fitted.chars().count(), 20);
        assert!(fitted.ends_with('…'));
    }
}
