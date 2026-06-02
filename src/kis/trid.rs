use crate::kis::config::Environment;
use crate::kis::error::{KisError, Result};

/// 실전/모의 tr_id 쌍. 시세계 GET은 실전·모의 동일(`same`),
/// 일부 TR은 모의 미지원(`real_only`).
#[derive(Debug, Clone, Copy)]
pub struct TrId {
    pub real: &'static str,
    /// None = 모의투자 미지원.
    pub mock: Option<&'static str>,
}

impl TrId {
    /// 실전/모의 tr_id가 다른 경우.
    pub const fn both(real: &'static str, mock: &'static str) -> Self {
        Self {
            real,
            mock: Some(mock),
        }
    }

    /// 실전/모의 tr_id가 동일한 경우 (시세계 GET API).
    pub const fn same(id: &'static str) -> Self {
        Self {
            real: id,
            mock: Some(id),
        }
    }

    /// 모의투자 미지원 TR.
    pub const fn real_only(real: &'static str) -> Self {
        Self { real, mock: None }
    }

    /// 환경에 맞는 tr_id 반환. 모의 미지원이면 `UnsupportedInMock`.
    pub fn resolve(&self, env: Environment) -> Result<&'static str> {
        match env {
            Environment::Real => Ok(self.real),
            Environment::Mock => self.mock.ok_or_else(|| KisError::UnsupportedInMock {
                tr_id: self.real.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_resolves_per_env() {
        let tr = TrId::both("TTTC0012U", "VTTC0012U");
        assert_eq!(tr.resolve(Environment::Real).unwrap(), "TTTC0012U");
        assert_eq!(tr.resolve(Environment::Mock).unwrap(), "VTTC0012U");
    }

    #[test]
    fn same_resolves_identical() {
        let tr = TrId::same("FHKST01010100");
        assert_eq!(tr.resolve(Environment::Real).unwrap(), "FHKST01010100");
        assert_eq!(tr.resolve(Environment::Mock).unwrap(), "FHKST01010100");
    }

    #[test]
    fn real_only_errors_in_mock() {
        let tr = TrId::real_only("TTTS3018R");
        assert!(tr.resolve(Environment::Real).is_ok());
        assert!(matches!(
            tr.resolve(Environment::Mock),
            Err(KisError::UnsupportedInMock { .. })
        ));
    }
}
