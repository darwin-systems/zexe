use crate::mnt4_753::{self, Fq, Fr, FR_ONE};
use algebra_core::{
    biginteger::BigInteger768,
    curves::{
        mnt4,
        models::{ModelParameters, SWModelParameters},
    },
    field_new, impl_scalar_mul_kernel,
};

pub type G1Affine = mnt4::G1Affine<mnt4_753::Parameters>;
pub type G1Projective = mnt4::G1Projective<mnt4_753::Parameters>;
pub type G1Prepared = mnt4::G1Prepared<mnt4_753::Parameters>;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Parameters;

impl ModelParameters for Parameters {
    type BaseField = Fq;
    type ScalarField = Fr;
}

impl_scalar_mul_kernel!(mnt4_753, "mnt4_753", g1, G1Projective);

impl SWModelParameters for Parameters {
    /// COEFF_A = 2
    #[rustfmt::skip]
    const COEFF_A: Fq = field_new!(Fq, BigInteger768([
            3553860551672651396,
            2565472393707818253,
            3424927325234966109,
            17487811826058095619,
            15730291918544907998,
            4332070408724822737,
            7212646118208244402,
            12904649141092619460,
            9289117987390442562,
            2254330573517213976,
            3065472942259520298,
            271095073719429,
    ]));

    /// COEFF_B = 0x01373684A8C9DCAE7A016AC5D7748D3313CD8E39051C596560835DF0C9E50A5B59B882A92C78DC537E51A16703EC9855C77FC3D8BB21C8D68BB8CFB9DB4B8C8FBA773111C36C8B1B4E8F1ECE940EF9EAAD265458E06372009C9A0491678EF4
    #[rustfmt::skip]
    const COEFF_B: Fq = field_new!(Fq, BigInteger768([
            2672638521926201442,
            17587766986973859626,
            1309143029066506763,
            1756412671449422902,
            5395165286423163724,
            589638022240022974,
            7360845090332416697,
            9829497896347590557,
            9341553552113883496,
            5888515763059971584,
            10173739464651404689,
            456607542322059,
    ]));

    /// COFACTOR = 1
    const COFACTOR: &'static [u64] = &[1];

    /// COFACTOR^(-1) mod r =
    /// 1
    #[rustfmt::skip]
    const COFACTOR_INV: Fr = FR_ONE;

    /// AFFINE_GENERATOR_COEFFS = (G1_GENERATOR_X, G1_GENERATOR_Y)
    const AFFINE_GENERATOR_COEFFS: (Self::BaseField, Self::BaseField) =
        (G1_GENERATOR_X, G1_GENERATOR_Y);

    fn scalar_mul_kernel(
        ctx: &Context,
        grid: usize,
        block: usize,
        table: *const G1Projective,
        exps: *const u8,
        out: *mut G1Projective,
        n: isize,
    ) -> error::Result<()> {
        scalar_mul(ctx, grid, block, (table, exps, out, n))
    }
}

// Generator of G1
// X = 7790163481385331313124631546957228376128961350185262705123068027727518350362064426002432450801002268747950550964579198552865939244360469674540925037890082678099826733417900510086646711680891516503232107232083181010099241949569,
// Y = 6913648190367314284606685101150155872986263667483624713540251048208073654617802840433842931301128643140890502238233930290161632176167186761333725658542781350626799660920481723757654531036893265359076440986158843531053720994648,
/// G1_GENERATOR_X =
#[rustfmt::skip]
pub const G1_GENERATOR_X: Fq = field_new!(Fq, BigInteger768([
    9433494781491502420,
    373642694095780604,
    7974079134466535382,
    15325904219470166885,
    16825705122208020751,
    898733863352481713,
    3802318585082797759,
    14417069684372068941,
    4332882897981414838,
    15138727514183191816,
    16850594895992448907,
    30598511593902
]));

/// G1_GENERATOR_Y =
#[rustfmt::skip]
pub const G1_GENERATOR_Y: Fq = field_new!(Fq, BigInteger768([
    15710199097794077134,
    3645667958306606136,
    8298269426007169475,
    5277073422205725562,
    10451808582969862130,
    14392820246664025579,
    4365987620174557815,
    14007263953321073101,
    1355600847400958219,
    3872959105252355444,
    18016882244107198324,
    424779036457857
]));
