use std::convert::TryFrom;
use std::ops::Neg;

use bls12_381::{G1Projective, G2Projective, Scalar};
use group::Curve;

use crate::Attribute;
use crate::scheme::setup::GroupParameters;

#[derive(Debug, Clone)]
pub struct SPSVerificationKey {
    pub grp: GroupParameters,
    pub uus: Vec<G1Projective>,
    pub wws: Vec<G2Projective>,
    pub yy: G2Projective,
    pub zz: G2Projective,
}

pub struct SPSSecretKey {
    sps_vk: SPSVerificationKey,
    us: Vec<Scalar>,
    ws: Vec<Scalar>,
    y: Scalar,
    z: Scalar,
}

impl SPSSecretKey {
    pub fn z(&self) -> Scalar {
        self.z
    }

    pub fn y(&self) -> Scalar {
        self.y
    }

    pub fn sign(&self, grp: GroupParameters, messages_a: Option<&[G1Projective]>, messages_b: Option<&[G2Projective]>) -> SPSSignature {
        let r = grp.random_scalar();
        let rr = grp.gen1() * r;
        let ss: G1Projective = match messages_a {
            Some(msgsA) => {
                let prodS: Vec<G1Projective> = msgsA
                    .iter()
                    .zip(self.ws.iter())
                    .map(|(m_i, w_i)| m_i * w_i.neg())
                    .collect();
                grp.gen1() * (self.z() - r * self.y()) + prodS.iter().fold(G1Projective::identity(), |acc, elem| acc + elem)
            }
            None => grp.gen1() * (self.z() - r * self.y())
        };
        let tt = match messages_b {
            Some(msgsB) => {
                let prodT: Vec<G2Projective> = msgsB
                    .iter()
                    .zip(self.us.iter())
                    .map(|(m_i, u_i)| m_i * u_i.neg())
                    .collect();
                (grp.gen2() + prodT.iter().fold(G2Projective::identity(), |acc, elem| acc + elem)) * r.invert().unwrap()
            }
            None => grp.gen2() * r.invert().unwrap()
        };

        SPSSignature
        {
            rr,
            ss,
            tt,
        }
    }
}

impl SPSVerificationKey {
    pub fn verify() -> bool {
        return true;
    }

    pub fn get_ith_ww(&self, idx: usize) -> &G2Projective { return self.wws.get(idx).unwrap(); }

    pub fn get_zz(&self) -> &G2Projective { return &self.zz; }

    pub fn get_yy(&self) -> &G2Projective { return &self.yy; }
}

pub struct SPSKeyPair {
    pub sps_sk: SPSSecretKey,
    pub sps_vk: SPSVerificationKey,
}

impl SPSKeyPair {
    pub fn new(grparams: GroupParameters, a: usize, b: usize) -> SPSKeyPair {
        let us = grparams.n_random_scalars(b);
        let ws = grparams.n_random_scalars(a);
        let y = grparams.random_scalar();
        let z = grparams.random_scalar();
        let uus: Vec<G1Projective> = us.iter().map(|u| grparams.gen1() * u).collect();
        let yy = grparams.gen2() * y;
        let wws: Vec<G2Projective> = ws.iter().map(|w| grparams.gen2() * w).collect();
        let zz = grparams.gen2() * z;

        let sps_vk = SPSVerificationKey {
            grp: grparams.clone(),
            uus,
            wws,
            yy,
            zz,
        };
        let sps_sk = SPSSecretKey {
            sps_vk: sps_vk.clone(),
            us,
            ws,
            y,
            z,
        };
        SPSKeyPair { sps_sk, sps_vk }
    }
}

#[derive(Debug, Clone)]
pub struct SPSSignature {
    pub rr: G1Projective,
    pub ss: G1Projective,
    pub tt: G2Projective,
}
