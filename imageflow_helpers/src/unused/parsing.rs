#![allow(bad_style)]
#![allow(unused, deprecated)]
use crate::preludes::from_std::*;
use num::{One, Zero};
use num::bigint::{BigInt, Sign};
use sha2::{Sha512, Digest};
use chrono::{DateTime, Utc};
use unicase::UniCase;
use crate::errors::*;
use crate::errors::Result;
use smallvec::SmallVec;
use std::iter::FromIterator;
use std::ascii::AsciiExt;

use super::super::util::*;

#[derive(Debug, Clone)]
pub struct LicenseParser{
    id: String,
    issued: Option<DateTime<Utc>>,
    expires: Option<DateTime<Utc>>,
    subscription_expiration_date: Option<DateTime<Utc>>,
    pairs: HashMap<AsciiFolding<String>,String>,
    original_length: usize
}

impl LicenseParser{
    pub fn new(s: &str) -> Result<LicenseParser>{

        let mut pairs = HashMap::new();
        for line in s.lines(){
            if let Some(colon) = line.find(":") {
                let key = AsciiFolding::new(line[..colon].trim().to_owned());
                let value = line[colon+1..].trim();
                pairs.insert(key, value.to_owned());
            }
        }
        let mut parser = LicenseParser {
            id: String::new(),
            issued: None,
            expires: None,
            subscription_expiration_date: None,
            pairs,
            original_length: s.len()
        };
        parser.id = parser.get("Id").or_else(||parser.get("Domain")).expect("No 'Id' or 'Domain' fields found in license. At least one is required.").to_owned();

        if parser.is_remote_placeholder(){
            let _ = parser.secret().and_then(|s| if s.chars().all(|c|c.is_ascii_whitespace()) { None } else {Some(s)}).expect("Placeholder license (Kind: id) is missing secret.");
        }

        parser.issued = parser.parse_date("Issued");
        parser.expires = parser.parse_date("Expires");
        parser.subscription_expiration_date = parser.parse_date("SubscriptionExpirationDate");
        Ok(parser)

    }

    pub fn get(&self, key: &str) -> Option<&str>{
        self.pairs.get(AsciiFolding::borrowed(key)).map(|s| s.as_ref())
    }

    pub fn iter(&self) -> ::std::collections::hash_map::Iter<AsciiFolding<String>,String>{
        self.pairs.iter()
    }


    fn parse_date(&self, key: &str) -> Option<DateTime<Utc>>{
        self.get(key).map(|s| DateTime::parse_from_rfc3339(s).expect("License dates must be valid iso1806/rfc3339 date strings.").with_timezone(&::chrono::Utc))
    }

    fn parse_int(&self, key: &str) -> Option<i32> {
        self.get(key).and_then(|s| s.parse().ok())
    }

    pub fn id(&self) -> &str{
        &self.id
    }
    pub fn issued(&self) -> Option<DateTime<Utc>>{
        self.issued
    }
    pub fn expires(&self) -> Option<DateTime<Utc>>{
        self.expires
    }
    pub fn subscription_expiration_date(&self) -> Option<DateTime<Utc>>{
        self.subscription_expiration_date
    }
    pub fn is_remote_placeholder(&self) -> bool{
        self.get("Kind").map(|s| UniCase::new(s) == UniCase::new("id")).unwrap_or(false)
    }
    pub fn is_revoked(&self) -> bool{
        self.get("Valid").map(|s| UniCase::new(s) == UniCase::new("false")).unwrap_or(false)
    }
    pub fn is_public(&self) -> bool{
        self.get("IsPublic").map(|s| UniCase::new(s) == UniCase::new("true")).unwrap_or(false)
    }
    pub fn is_must_be_fetched(&self) -> bool{
        self.get("MustBeFetched").map(|s| UniCase::new(s) == UniCase::new("true")).unwrap_or(false)
    }
    pub fn network_grace_minutes(&self) -> Option<i32>{
        self.parse_int("NetworkGraceMinutes")
    }
    pub fn check_license_interval_minutes(&self) -> Option<i32>{
        self.parse_int("CheckLicenseIntervalMinutes")
    }
    pub fn secret(&self) -> Option<&str>{
        self.get("Secret")
    }
    pub fn restrictions(&self) -> Option<&str>{
        self.get("Restrictions")
    }
    pub fn message(&self) -> Option<&str>{
        self.get("Message")
    }
    pub fn expiry_message(&self) -> Option<&str>{
        self.get("ExpiryMessage")
    }
    pub fn features(&self) -> SmallVec<[UniCase<&str>;4]>{
        let mut vec = SmallVec::new();
        if let Some(features) = self.get("Features") {
            for v in features.split(&[' ', '\t', ','][..]) {
                vec.push(UniCase::new(v.trim()));
            }
        }
        vec
    }
    pub fn domains(&self) -> SmallVec<[UniCase<&str>;2]>{
        let mut vec: SmallVec<[UniCase<&str>;2]> =  SmallVec::new();
        if let Some(domains) =self.get("Domains"){
            for d in domains.split(&[' ', '\t', ','][..]){
                vec.push(UniCase::new(d.trim()));
            }
        }
        if let Some(d) = self.get("Domain"){
            vec.push(UniCase::new(d.trim()));
        }
        vec.retain(|d| d.len() > 0);
        vec
    }

    pub fn domains_owned(&self) -> SmallVec<[UniCase<String>;2]>{
        SmallVec::from_iter(self.domains().into_iter().map(|s| UniCase::new(s.as_ref().to_owned())))
    }

    pub fn license_servers(&self) -> SmallVec<[&str;4]>{
        let mut vec = SmallVec::new();
        if let Some(features) = self.get("LicenseServers") {
            for v in features.split(&[' ', '\t'][..]) {
                vec.push(v.trim());
            }
        }
        vec
    }


    pub fn to_redacted_str(&self) -> String{
        let mut result = String::with_capacity(self.original_length + 20);
        for (k,v) in self.iter(){
            if k.as_ref().eq_ignore_ascii_case("secret"){
                result.push_str("secret: ****redacted****");
            }else{
                result.push_str(k.as_ref());
                result.push_str(": ");
                result.push_str(v);
                result.push_str("\n")
            }
        }
        result
    }

    pub fn to_string(&self) -> String{
        let mut result = String::with_capacity(self.original_length + 20);
        for (k,v) in self.iter() {
            result.push_str(k.as_ref());
            result.push_str(": ");
            result.push_str(v);
            result.push_str("\n")
        }
        result
    }

}
#[derive(Debug, Clone)]
pub struct LicenseBlob{
//    original: String,
    signature: Vec<u8>,
//    data: Vec<u8>,
    fields: LicenseParser
}

impl LicenseBlob{
    pub fn fields(&self) -> &LicenseParser{
        &self.fields
    }

    pub fn id(&self) -> &str{
        self.fields.id()
    }

    pub fn deserialize(trusted_keys: &[RSADecryptPublic], license: &str, error_prefix: &str) -> Result<LicenseBlob>{
        let parts: SmallVec<[&str;4]> = SmallVec::from_iter(license.split(":").map(|s| s.trim()));
        if parts.len() < 2{
            return Err(Error::from_kind(ErrorKind::LicenseCorrupted(format!("{} Incomplete: not enough ':' delimited segments found.\n{}",error_prefix, license))));
        }

        // Allocation
        let data_bytes = ::base64::decode(parts[parts.len() - 2].as_bytes())
            .chain_err(|| Error::from_kind(ErrorKind::LicenseCorrupted(format!("{} Second-to-last segment is not valid base 64.\n{}",error_prefix, license))))?;

        // Allocation
        let signature_bytes = ::base64::decode(parts[parts.len() - 1].as_bytes())
            .chain_err(|| Error::from_kind(ErrorKind::LicenseCorrupted(format!("{} Second-to-last segment is not valid base 64.\n{}",error_prefix, license))))?;

        // Several allocations
        let signature_valid = LicenseBlob::validate_signature(&data_bytes, &signature_bytes, trusted_keys)
            .chain_err(|| Error::from_kind(ErrorKind::LicenseCorrupted(format!("{} Signature too long.\n{}",error_prefix, license))))?;

        if !signature_valid {
            return Err(Error::from_kind(ErrorKind::LicenseCorrupted(format!("{} Decrypted signature does not match license contents.\n{}",error_prefix, license))));
        }


        let fields = {
            let data_string = str::from_utf8(&data_bytes).chain_err(|| format!("{} License contents must be valid UTF-8 bytes", error_prefix))?;
            LicenseParser::new(data_string)? // Each key, each value, is an alloc (plus HashMap and Id)
        };
        Ok(
            LicenseBlob{
//                original: license.to_owned(),
                signature: signature_bytes,
//                data: data_bytes,
                fields
            }
        )
    }

    fn validate_signature(data: &[u8], signature_bytes: &[u8], trusted_keys: &[RSADecryptPublic]) -> Result<bool>{
        let mut hasher = Sha512::default();
        hasher.input(data);
        use ::digest::FixedOutput;
        let digest = hasher.fixed_result();

        let sig = RSAData::from(signature_bytes); // This allocates (BigInt)

        for rsa in trusted_keys{
            let decrypted = rsa.decrypt_public(&sig)?; // This allocates to do math (BigInt)
            if decrypted.as_ref() == digest.as_ref() {
                return Ok(true);
            }
        }
        Ok(false)
    }



}

pub struct RSADecryptPublic {
    modulus: BigInt,
    exponent: BigInt
}

impl RSADecryptPublic{
    pub fn new(modulus: BigInt, exponent: BigInt) -> Option<RSADecryptPublic>{
        // Furthermore, exponent should always be 65537
        if modulus > BigInt::zero() && exponent > BigInt::zero() {
            Some(
                RSADecryptPublic {
                    modulus,
                    exponent
                })
        }else{
            None
        }
    }
    pub fn from_byte_strings(modulus: &[u8], exponent: &[u8]) -> Option<RSADecryptPublic> {
        let m_maybe = BigInt::parse_bytes(modulus, 10);
        let e_maybe = BigInt::parse_bytes(exponent, 10);
        if let (Some(m), Some(e)) = (m_maybe, e_maybe) {
            RSADecryptPublic::new(m,e)
        } else {
            None
        }
    }

    fn powm(base: &BigInt, exp: &BigInt, modulus: &BigInt) -> BigInt {
        let zero: BigInt = BigInt::zero();
        let one: BigInt = BigInt::one();
        let two = &one + &one;
        let mut exp = exp.clone();
        let mut result = one.clone();
        let mut base = base % modulus;

        while exp > zero {
            if &exp % &two == one {
                result = (result * &base) % modulus;
            }
            exp = exp >> 1;
            base = (&base * &base) % modulus;
        }
        result
    }
    fn mod_pow(&self, input: &BigInt) -> BigInt{
        RSADecryptPublic::powm(input, &self.exponent, &self.modulus)
    }

    /// This method allocates quite a bit due to use of BigInt
    pub fn decrypt_public(&self, data: &RSAData) -> Result<SmallVec<[u8;64]>>{
        // RSA specifies big-endian
        // There's an upper bound on all of these allocations. Wish SmallVec was an option
        let input = data.data();
        if input >= &self.modulus{
            // input too long for RSA cipher block size
            Err(Error::from_kind(ErrorKind::RsaDecryptInputLargerThanModulus))
        }else {
            //.skip_while(|v| *v != 0).skip(1) skips padding
            Ok(SmallVec::from_iter(self.mod_pow(&input).to_bytes_be().1.into_iter().skip_while(|v| *v != 0).skip(1)))
        }
    }
}

pub struct RSAData(BigInt);
impl RSAData{
    pub fn from(bytes: &[u8]) -> RSAData{
        // This allocates a vector to reverse, then allocates again for the integer
        RSAData(BigInt::from_bytes_be(Sign::Plus, bytes))
    }
    pub fn data(&self) -> &BigInt{
        &self.0
    }
}

fn get_production_keys() -> Vec<RSADecryptPublic> {
    vec![RSADecryptPublic::from_byte_strings(
        b"23949488589991837273662465276682907968730706102086698017736172318753209677546629836371834786541857453052840819693021342491826827766290334135101781149845778026274346770115575977554682930349121443920608458091578262535319494351868006252977941758848154879863365934717437651379551758086088085154566157115250553458305198857498335213985131201841998493838963767334138323078497945594454883498534678422546267572587992510807296283688571798124078989780633040004809178041347751023931122344529856055566400640640925760832450260419468881471181281199910469396775343083815780600723550633987799763107821157001135810564362648091574582493",
        b"65537").unwrap(),
         RSADecryptPublic::from_byte_strings(
             b"20966000569757071862106887100142448229133877611190126160168597284259733824510172534126967070490592659952430888203435031779696121874348777439846786968121542858840906429510085119585674950522992116110440180288728612219347325636018396716507682924594303420147925518492731883007123328081986113438120311956235689236820190735716844178839961449198918585485277306636638238163410140728079481083558191670535479781738412622557832581113291858559860935145319768483825412681366230852014952837750160226558508220374106696447994610354318517561059830141995002511253671974534953764078640650030953288533566233172651498868658899945417935381",
             b"65537").unwrap()]

}

fn get_test_keys() -> Vec<RSADecryptPublic> {
    vec![RSADecryptPublic::from_byte_strings(
        b"21403964489040138713896545869406851734432500305180577929806228393671667423170541918856531956008546071841016201645150244452266439995041173092354230946610429300967887006960186647111152810965360763586210200652502467947786453111507369142658284220331513416234497960844309808252643534631142917589553418044306073242485021092396181183125381004682521853943025560860753079004948017667604884278401445729443478586697229583656851019218046599746243419376456426788044497274378001221965538712352348475726349124652450874653832672820100829574087311416068166524423905971193163418806721436095962165082262760557869093554827824418663362349",
        b"65537").unwrap()]
}


lazy_static!{

        pub static ref TEST_KEYS: Vec<RSADecryptPublic> = get_test_keys();

//        pub static ref PRODUCTION_KEYS: Vec<RSADecryptPublic> = get_production_keys();
//        pub static ref ALL_KEYS: Vec<RSADecryptPublic> = {
//            get_production_keys().into_iter().chain(get_test_keys().into_iter()).collect()
//        };
    }

#[cfg(test)]
mod test{

    use super::*;

    #[test]
    fn test_generic(){

        let rsa = RSADecryptPublic::from_byte_strings(b"28178177427582259905122756905913963624440517746414712044433894631438407111916149031583287058323879921298234454158166031934230083094710974550125942791690254427377300877691173542319534371793100994953897137837772694304619234054383162641475011138179669415510521009673718000682851222831185756777382795378538121010194881849505437499638792289283538921706236004391184253166867653735050981736002298838523242717690667046044130539971131293603078008447972889271580670305162199959939004819206804246872436611558871928921860176200657026263241409488257640191893499783065332541392967986495144643652353104461436623253327708136399114561",
                                                      b"65537").expect("RSA parameters must be positive integers in base 10");


        let blob = LicenseBlob::deserialize(&[rsa],"localhost:RG9tYWluOiBsb2NhbGhvc3QKT3duZXI6IEV2ZXJ5b25lCklzc3VlZDogMjAxNS0wMy0yOFQwOTozNjo1OVoKRmVhdHVyZXM6IFI0RWxpdGUgUjRDcmVhdGl2ZSBSNFBlcmZvcm1hbmNlCg==:h6D+kIXbF3qmvmW2gDpb+b4gdxBjnrkZLvSzXmEnqKAywNJNpTdFekpTOB4SwU14WbTeVyWwvFngHax7WuHBV+0WkQ5lDqKFaRW32vj8CJQeG8Wvnyj9PaNGaS/FpKhNjZbDEmh3qqirBp2NR0bpN4QbhP9NMy7+rOMo0nynAruwWvJKCnuf7mWWdb9a5uTZO9OUcSeS/tY8QaNeIhaCnhPe0Yx9qvOXe5nMnl10CR9ur+EtS54d1qzBGHqN/3oFhiB+xlqNELwz23qR4c8HxbTEyNarkG4CZx8CbbgJfHmPxAYGJTTBTPJ+cdah8MJR16Ta36cRZ2Buy8XYo/nf1g==", "test_generic");
        if let Err(e) = blob{
            use error_chain::ChainedError;
            eprintln!("{}",e
            );
            assert!(false);
        }
    }
}
