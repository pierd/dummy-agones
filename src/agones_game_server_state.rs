// From Agones source code: https://github.com/googleforgames/agones/blob/cef40425076e5e9a766952eb2f499d9b447b703a/pkg/apis/agones/v1/gameserver.go#L36
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum GameServerState {
    /// PortAllocation is for when a dynamically allocating GameServer
    /// is being created, an open port needs to be allocated
    PortAllocation,
    /// Creating is before the Pod for the GameServer is being created
    Creating,
    /// Starting is for when the Pods for the GameServer are being
    /// created but are not yet Scheduled
    Starting,
    /// Scheduled is for when we have determined that the Pod has been
    /// scheduled in the cluster -- basically, we have a NodeName
    Scheduled,
    /// RequestReady is when the GameServer has declared that it is ready
    RequestReady,
    /// Ready is when a GameServer is ready to take connections
    /// from Game clients
    Ready,
    /// Shutdown is when the GameServer has shutdown and everything needs to be
    /// deleted from the cluster
    Shutdown,
    /// Error is when something has gone wrong with the Gameserver and
    /// it cannot be resolved
    Error,
    /// Unhealthy is when the GameServer has failed its health checks
    Unhealthy,
    /// Reserved is for when a GameServer is reserved and therefore can be allocated but not removed
    Reserved,
    /// Allocated is when the GameServer has been allocated to a session
    Allocated,
}

impl std::str::FromStr for GameServerState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PortAllocation" => Ok(Self::PortAllocation),
            "Creating" => Ok(Self::Creating),
            "Starting" => Ok(Self::Starting),
            "Scheduled" => Ok(Self::Scheduled),
            "RequestReady" => Ok(Self::RequestReady),
            "Ready" => Ok(Self::Ready),
            "Shutdown" => Ok(Self::Shutdown),
            "Error" => Ok(Self::Error),
            "Unhealthy" => Ok(Self::Unhealthy),
            "Reserved" => Ok(Self::Reserved),
            "Allocated" => Ok(Self::Allocated),
            _ => Err(format!("Unknown GameServerState: {}", s)),
        }
    }
}

impl std::fmt::Display for GameServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameServerState::PortAllocation => write!(f, "PortAllocation"),
            GameServerState::Creating => write!(f, "Creating"),
            GameServerState::Starting => write!(f, "Starting"),
            GameServerState::Scheduled => write!(f, "Scheduled"),
            GameServerState::RequestReady => write!(f, "RequestReady"),
            GameServerState::Ready => write!(f, "Ready"),
            GameServerState::Shutdown => write!(f, "Shutdown"),
            GameServerState::Error => write!(f, "Error"),
            GameServerState::Unhealthy => write!(f, "Unhealthy"),
            GameServerState::Reserved => write!(f, "Reserved"),
            GameServerState::Allocated => write!(f, "Allocated"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_to_string_should_be_parsed_to_the_same_state_by_from_str() {
        for state in [
            GameServerState::PortAllocation,
            GameServerState::Creating,
            GameServerState::Starting,
            GameServerState::Scheduled,
            GameServerState::RequestReady,
            GameServerState::Ready,
            GameServerState::Shutdown,
            GameServerState::Error,
            GameServerState::Unhealthy,
            GameServerState::Reserved,
            GameServerState::Allocated,
        ] {
            let state = state.to_string();
            assert_eq!(state.parse::<GameServerState>().unwrap().to_string(), state);
        }
    }

    #[test]
    fn state_from_str_should_generate_the_same_string_with_to_string() {
        for state_str in [
            "PortAllocation",
            "Creating",
            "Starting",
            "Scheduled",
            "RequestReady",
            "Ready",
            "Shutdown",
            "Error",
            "Unhealthy",
            "Reserved",
            "Allocated",
        ] {
            let state = state_str.parse::<GameServerState>().unwrap();
            assert_eq!(state.to_string(), state_str);
        }
    }
}
