use sui_sdk_types::Address;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveTable {
    pub id: Address,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRegistryMove {
    pub id: Address,
    pub accounts: MoveTable,
}

const REGISTRY_FIELD_VALUE_OFFSET: usize = Address::LENGTH + Address::LENGTH;
const REGISTRY_FIELD_PREFIX_LENGTH: usize = REGISTRY_FIELD_VALUE_OFFSET + Address::LENGTH;

impl AccountRegistryMove {
    const REGISTRY_PREFIX_LENGTH: usize = Address::LENGTH + Address::LENGTH + size_of::<u64>();

    pub fn from_bcs_prefix(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < Self::REGISTRY_PREFIX_LENGTH {
            return Err(format!(
                "registry bcs is too short: expected at least {} bytes, got {}",
                Self::REGISTRY_PREFIX_LENGTH,
                bytes.len()
            ));
        }

        let id = read_address(bytes, 0, "registry id")?;
        let accounts_id = read_address(bytes, Address::LENGTH, "accounts table id")?;
        let size_offset = Address::LENGTH * 2;
        let size_bytes = bytes
            .get(size_offset..size_offset + size_of::<u64>())
            .ok_or_else(|| "registry bcs is missing accounts table size".to_owned())?;
        let size = u64::from_le_bytes(
            size_bytes
                .try_into()
                .map_err(|_| "invalid accounts table size".to_owned())?,
        );

        Ok(Self {
            id,
            accounts: MoveTable {
                id: accounts_id,
                size,
            },
        })
    }
}

pub fn account_id_from_registry_field_bcs(bytes: &[u8]) -> Result<Address, String> {
    if bytes.len() < REGISTRY_FIELD_PREFIX_LENGTH {
        return Err(format!(
            "registry field bcs is too short: expected at least {} bytes, got {}",
            REGISTRY_FIELD_PREFIX_LENGTH,
            bytes.len()
        ));
    }

    read_address(bytes, REGISTRY_FIELD_VALUE_OFFSET, "registry account id")
}

fn read_address(bytes: &[u8], offset: usize, field: &str) -> Result<Address, String> {
    let end = offset + Address::LENGTH;
    let address_bytes = bytes
        .get(offset..end)
        .ok_or_else(|| format!("registry bcs is missing {field}"))?;
    Address::from_bytes(address_bytes).map_err(|_| format!("invalid {field} address bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_table_from_prefix_with_trailing_fields() {
        let registry_id = Address::from_static(
            "0x0da982cefa26864ae834a8a0504b904233d49e20fcc17c373c8bed99c75a7edd",
        );
        let accounts_id = Address::from_static(
            "0xf3ca7bea4cf3f5db23a2128391c85878579d581496850956ba3de07017b92229",
        );
        let size = 7u64;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(registry_id.as_bytes());
        bytes.extend_from_slice(accounts_id.as_bytes());
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.extend_from_slice(&[1, 2, 3, 4]);

        let registry = AccountRegistryMove::from_bcs_prefix(&bytes).unwrap();

        assert_eq!(
            registry,
            AccountRegistryMove {
                id: registry_id,
                accounts: MoveTable {
                    id: accounts_id,
                    size,
                },
            }
        );
    }

    #[test]
    fn rejects_short_registry_bcs() {
        let bytes = vec![0; AccountRegistryMove::REGISTRY_PREFIX_LENGTH - 1];

        assert!(AccountRegistryMove::from_bcs_prefix(&bytes).is_err());
    }

    #[test]
    fn parses_account_id_from_registry_field_bcs_prefix() {
        let field_id = Address::from_static(
            "0x8e4af656c955e9fd1e0585d81f07cfe6a3b4e6969fd9cc5d9149100c8cd1e98b",
        );
        let wallet = Address::from_static(
            "0x0ab64fabc3770c8fe4b00547e5f53e27963fb29239b9bf01761a7552139a0546",
        );
        let account_id = Address::from_static(
            "0x06e69014c08be834d8bccbde2a61992a5810dcf966f9a9d5e502bf0afccfc210",
        );
        let mut bytes = Vec::new();
        bytes.extend_from_slice(field_id.as_bytes());
        bytes.extend_from_slice(wallet.as_bytes());
        bytes.extend_from_slice(account_id.as_bytes());
        bytes.extend_from_slice(&[1, 2, 3, 4]);

        assert_eq!(
            account_id_from_registry_field_bcs(&bytes).unwrap(),
            account_id
        );
    }
}
