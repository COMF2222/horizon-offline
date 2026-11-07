//! Local skins shared by Horizon clients through their existing game connection.
//! The wire carries a fixed 64×64 RGBA image, never a path, URL, or image decoder input.
use crate::Error;

pub const CHANNEL: &str = "horizon:offline_skin_v1";
pub const VERSION: u8 = 1;
pub const RGBA_LEN: usize = 64 * 64 * 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skin {
    pub rgba: Vec<u8>,
    pub slim: bool,
}

/// Advertises support, with the selected local skin when its decoded dimensions are valid.
/// There is no UUID in an upload: only the server assigns the sender's identity.
pub fn encode_client(skin: Option<&Skin>) -> Vec<u8> {
    let Some(skin) = skin.filter(|skin| skin.rgba.len() == RGBA_LEN) else {
        return vec![VERSION, 0];
    };
    let mut data = Vec::with_capacity(RGBA_LEN + 3);
    data.extend_from_slice(&[VERSION, 1, u8::from(skin.slim)]);
    data.extend_from_slice(&skin.rgba);
    data
}

/// Rejects unknown versions, invalid flags, and any truncated or oversized image before copying it.
pub fn decode_client(data: &[u8]) -> Result<Option<Skin>, Error> {
    match data {
        [VERSION, 0] => Ok(None),
        [VERSION, 1, model @ (0 | 1), rgba @ ..] if rgba.len() == RGBA_LEN => Ok(Some(Skin {
            rgba: rgba.to_vec(),
            slim: *model == 1,
        })),
        _ => Err(Error::new("Invalid Horizon local skin payload")),
    }
}

/// A server's update, bound to a player's real connection UUID.
pub fn encode_server(uuid: u128, skin: Option<&Skin>) -> Vec<u8> {
    let mut data = Vec::with_capacity(16 + RGBA_LEN + 3);
    data.extend_from_slice(&uuid.to_be_bytes());
    data.extend_from_slice(&encode_client(skin));
    data
}

pub fn decode_server(data: &[u8]) -> Result<(u128, Option<Skin>), Error> {
    let Some((uuid, skin)) = data.split_at_checked(16) else {
        return Err(Error::new("Truncated Horizon local skin UUID"));
    };
    let uuid = u128::from_be_bytes(
        uuid.try_into()
            .map_err(|_| Error::new("Invalid Horizon local skin UUID"))?,
    );
    Ok((uuid, decode_client(skin)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected_skin(slim: bool) -> Skin {
        Skin {
            rgba: [128, 64, 32, 255].repeat(4096),
            slim,
        }
    }

    #[test]
    fn a_selected_skin_has_a_bounded_image_and_explicit_model_on_the_wire() {
        for (slim, model) in [(false, 0), (true, 1)] {
            let skin = selected_skin(slim);
            let encoded = encode_client(Some(&skin));
            assert_eq!(&encoded[..7], &[1, 1, model, 128, 64, 32, 255]);
            assert_eq!(encoded.len(), 16387);
            assert_eq!(decode_client(&encoded).unwrap(), Some(skin));
        }
    }

    #[test]
    fn default_skin_still_announces_support_without_an_image() {
        assert_eq!(encode_client(None), [1, 0]);
        assert_eq!(decode_client(&[1, 0]).unwrap(), None);
        assert_eq!(
            encode_client(Some(&Skin {
                rgba: vec![0; 3],
                slim: false,
            })),
            [1, 0]
        );
    }

    #[test]
    fn malformed_uploads_never_become_images() {
        let valid = encode_client(Some(&selected_skin(false)));
        for data in [
            vec![],
            vec![1],
            vec![2, 0],
            vec![1, 2],
            vec![1, 0, 0],
            [&[1, 1, 2][..], &valid[3..]].concat(),
            valid[..valid.len() - 1].to_vec(),
            [valid.as_slice(), &[0]].concat(),
        ] {
            assert!(decode_client(&data).is_err());
        }
    }

    #[test]
    fn server_updates_bind_the_skin_to_the_named_player() {
        let uuid = 0x00112233445566778899aabbccddeeff;
        let skin = selected_skin(true);
        let encoded = encode_server(uuid, Some(&skin));
        assert_eq!(
            &encoded[..19],
            &[
                0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255, 1, 1, 1
            ]
        );
        assert_eq!(decode_server(&encoded).unwrap(), (uuid, Some(skin)));
        assert_eq!(
            decode_server(&encode_server(uuid, None)).unwrap(),
            (uuid, None)
        );
        assert!(decode_server(&encoded[..15]).is_err());
        assert!(decode_server(&[encoded.as_slice(), &[0]].concat()).is_err());
    }
}
