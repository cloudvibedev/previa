use crate::models::AwsNodeProfile;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct KarpenterPlan {
    pub node_pool: String,
    pub ec2_node_class: String,
    pub requirements: Vec<(String, Vec<String>)>,
}

#[allow(dead_code)]
pub fn build_aws_karpenter_plan(profile: &AwsNodeProfile) -> KarpenterPlan {
    KarpenterPlan {
        node_pool: profile.node_pool.clone(),
        ec2_node_class: profile.ec2_node_class.clone(),
        requirements: vec![
            (
                "karpenter.k8s.aws/instance-family".to_owned(),
                profile.instance_families.clone(),
            ),
            (
                "karpenter.k8s.aws/instance-size".to_owned(),
                profile.instance_sizes.clone(),
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::{AwsNodeProfile, build_aws_karpenter_plan};

    #[test]
    fn maps_aws_profile_to_karpenter_requirements() {
        let plan = build_aws_karpenter_plan(&AwsNodeProfile {
            node_pool: "previa-runner-small".to_owned(),
            ec2_node_class: "previa-runner".to_owned(),
            instance_families: vec!["t4g".to_owned(), "c7g".to_owned()],
            instance_sizes: vec!["nano".to_owned(), "micro".to_owned()],
            expire_after: "10m".to_owned(),
            consolidate_after: "30s".to_owned(),
        });

        assert_eq!(plan.node_pool, "previa-runner-small");
        assert_eq!(plan.ec2_node_class, "previa-runner");
        assert!(plan.requirements.iter().any(|(key, values)| {
            key == "karpenter.k8s.aws/instance-family" && values == &vec!["t4g", "c7g"]
        }));
    }
}
