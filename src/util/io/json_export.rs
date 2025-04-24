use jagua_rs::entities::strip_packing::{SPInstance, SPSolution};
use jagua_rs::io::export::export_spp_solution;
use jagua_rs::io::json_instance::JsonInstance;
use jagua_rs::io::json_solution::JsonSolution;
use serde::{Deserialize, Serialize};
use crate::EPOCH;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct JsonOutput {
    #[serde(flatten)]
    pub input: JsonInstance,
    pub solution: JsonSolution,
}

impl JsonOutput {
    pub fn new(json_instance: JsonInstance, solution: &SPSolution, instance: &SPInstance) -> Self {
        let json_solution = export_spp_solution(&solution, &instance, *EPOCH);
        JsonOutput {
            input: json_instance,
            solution: json_solution,
        }
    }
}