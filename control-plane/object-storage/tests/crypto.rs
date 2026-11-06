use object_storage::crypto::SecretBox;

const KEY_VAR: &str = "OBJECT_STORAGE_ENCRYPTION_KEY";
const VALID_KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

#[test]
fn secret_box_round_trips_and_rejects_bad_input() {
    std::env::remove_var(KEY_VAR);
    assert!(SecretBox::from_env().is_err());

    for invalid in ["abc", "zz", &"ab".repeat(16), &"gg".repeat(32)] {
        std::env::set_var(KEY_VAR, invalid);
        assert!(SecretBox::from_env().is_err(), "{}", invalid);
    }

    std::env::set_var(KEY_VAR, VALID_KEY);
    let secret_box = SecretBox::from_env().unwrap();
    let sealed = secret_box.encrypt("top-secret").unwrap();
    assert_eq!(secret_box.decrypt(&sealed).unwrap(), "top-secret");
    assert_ne!(sealed, secret_box.encrypt("top-secret").unwrap());

    assert!(secret_box.decrypt(&sealed[..5]).is_err());
    let mut tampered = sealed.clone();
    *tampered.last_mut().unwrap() ^= 0xff;
    assert!(secret_box.decrypt(&tampered).is_err());
}
