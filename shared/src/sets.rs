#[allow(non_snake_case)]
pub mod PreGameLoadingSets {
    use bevy::prelude::SystemSet;
    use derive_linear_schedule_set::LinearSystemSet;

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash, LinearSystemSet)]
    pub enum Update {
        Initialize,
        Networking,
        Rest,
    }

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash, LinearSystemSet)]
    pub enum OnEnter {
        Initialize,
        Networking,
        Resources,
        Ui,
        Rest,
    }
}

#[allow(non_snake_case)]
pub mod GameSets {
    use bevy::prelude::SystemSet;
    use derive_linear_schedule_set::LinearSystemSet;

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash, LinearSystemSet)]
    pub enum OnEnter {
        Initialize,
        Ui,
        Rest,
    }

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash, LinearSystemSet)]
    pub enum PreUpdate {
        PlayerInput,
        Rest,
    }

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash, LinearSystemSet)]
    pub enum Update {
        PlayerInput,
        PlayerPhysics,
        WorldInput,
        WorldPhysics,
        Networking,
        Rendering,
        Ui,
        Rest,
    }

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash, LinearSystemSet)]
    pub enum FixedPreUpdate {
        Networking,
        Rest,
    }

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash, LinearSystemSet)]
    pub enum FixedUpdate {
        Networking,
        Rest,
    }

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash, LinearSystemSet)]
    pub enum PostUpdate {
        Rendering,
        Rest,
    }

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash, LinearSystemSet)]
    pub enum OnExit {
        World,
        Networking,
        Rest,
    }
}
