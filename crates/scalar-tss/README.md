# Scalar Tss: A anemo threshold signature scheme daemon
Scalar Tss is a anemo server written in Rust that wraps the tofn threshold cryptography library.

## Mnemonic
Scalar Tss uses the tiny-bip39 crate to enable users manage mnemonic passphrases. Currently, each party can use only one passphrase.

Mnemonic is used to enable recovery of shares in case of unexpected loss. See more about recovery under the Recover section.

## KV Store
To persist information between different phases (i.e. keygen and sign), we use a key-value storage based on sled.

Scalar Tss uses two separate KV Stores:

Share KV Store. Stores all user's shares when keygen protocol is completed, and uses them for sign protocol. Default path is ./kvstore/shares.
Mnemonic KV Store. Stores the entropy of a mnemonic passphrase. This entropy is used to encrypt and decrypt users' sensitive info, i.e. the content of the Share KV Store. Default path is ./kvstore/mnemonic.
Security
Important note: Currently, the mnemonic KV Store is not encrypted. The mnemonic entropy is stored in clear text on disk. Our current security model assumes secure device access.

## Multiple shares
Multiple shares are handled internally. That is, if a party has 3 shares, the tofnd binary spawns 3 protocol execution threads, and each thread invokes tofn functions independently.

When a message is received from the client, it is broadcasted to all shares. This is done in the broadcast module.

At the end of the protocol, the outputs of all N party's shares are aggregated and a single result is created and sent to the client. There are separate modules keygen result and sign result that handles the aggregation results for each protocol.

## Services
Scalar Tss currently supports the following services:

- keygen
- sign
- recover
- key_presence

## Diagrams
See a generic protocol sequence diagram, [here](https://github.com/axelarnetwork/tofnd/blob/main/diagrams/protocol.pdf).

See [keygen](https://github.com/axelarnetwork/tofnd/blob/main/diagrams/keygen.svg) and [sign](https://github.com/axelarnetwork/tofnd/blob/main/diagrams/sign.svg) diagrams of detailed message flow of each protocol.

### Keygen
The keygen executes the keygen protocol as implemented in tofn and described in GG20.

### Successful keygen
On success, the keygen protocol returns a SecretKeyShare struct defined by tofn
```Rust
pub struct SecretKeyShare {
    group: GroupPublicInfo,
    share: ShareSecretInfo,
}
```
This struct includes:

The information that is needed by the party in order to participate in subsequent sign protocols that are associated with the completed keygen.
The public key of the current keygen.
Since multiple shares per party are supported, keygen's result may produce multiple SecretKeyShares. The collection of SecretKeyShares is stored in the Share KV Store as the value with the key_uid as key.

Each SecretKeyShare is then encrypted using the party's mnemonic, and the encrypted data is sent to the client as bytes, along with the public key. We send the encrypted SecretKeyShares to facilitate recovery in case of data loss.

The message of keygen's data is the following:
```Rust
message KeygenOutput {
    bytes pub_key = 1;                       // pub_key
    repeated bytes share_recovery_infos = 2; // recovery info
}
```
### Unsuccessful keygen
The tofn library supports fault detection. That is, if a party does not follow the protocol (e.g. by corrupting zero knowledge proofs, stalling messages etc), a fault detection mechanism is triggered, and the protocol ends prematurely with all honest parties composing a faulter list.

In this case, instead of the aforementioned result, keygen returns a Vec<Faulters>

### File structure
Keygen is implemented in scalar-tss/src/gg20/keygen, which has the following file structure:

├── keygen
    ├── mod.rs
    ├── init.rs
    ├── execute.rs
    ├── result.rs
    └── types.rs
In mod.rs, the handlers of protocol initialization, execution and aggregation of results are called. Also, in case of multiple shares, multiple execution threads are spawned.
In init.rs, the verification and sanitization of the Keygen Init message is handled.
In execute.rs, the instantiation and execution of the protocol is actualized.
In result.rs, the results of all party shares are aggregated, validated and sent to the client.
In types.rs, useful structs that are needed in the rest of the modules are defined.
Sign
The sign executes the sign protocol as implemented in tofn and described in GG20.

### Successful sign
On success, the keygen protocol returns a signature which is a Vec<u8>.

Since multiple shares per party are supported, sign's result may produce multiple signaturess which are the same across all shares. Only one copy of the signature is sent to the gRPC client.

### Unsuccessful sign
Similarly to keygen, if faulty parties are detected during the execution of sign, the protocol is stopped and a Vec<Faulters> is returned to the client.

### Trigger recovery
Sign is started with the special message SignInit.

key_uid indicates the session identifier of an executed keygen. In order to be able to participate to sign, parties need to have their share info stored at the Share KV Store as value, under the key key_uid. If this data is not present at the machine of a party (i.e. no key_uid exists in Share KV Store), a need_recover gRPC message is sent to the client and the connection is then closed. In the need_recover message, the missing key_uid is included.
```Rust
message NeedRecover {
    string session_id = 1;
}
```
The client then proceeds by triggering recover gRPC, and then starts the sign again for the recovered party. Other participants are not affected.

## File structure
The keygen protocol is implemented in scalar-tss/src/gg20/sign, which, similar to keygen, has the following file structure:

├── sign
    ├── mod.rs
    ├── init.rs
    ├── execute.rs
    ├── result.rs
    └── types.rs
In mod.rs, the handlers of protocol initialization, execution and aggregation of results are called. Also, in case of multiple shares, multiple execution threads are spawned.
In init.rs, the verification and sanitization of Sign Init message is handled. If the absence of shares is discovered, the client sends a need_recover and stops.
In execute.rs, the instantiation and execution of the protocol is actualized.
In result.rs, the results of all party shares are aggregated, validated and sent to the gRPC client.
In types.rs, useful structs that are needed in the rest of the modules are defined.
## Recover
As discussed in keygen and sign section, the recovery of lost keys and shares is supported. In case of sudden data loss, for example due to a hard disk crash, parties are able to recover their shares. This is possible because each party sends it's encrypted secret info to the client before storing it inside the Share KV Store.

When keygen is completed, the party's information is encryped and sent to the client. When the absence of party's information is detected during sign, Tofnd sends the need_recover message, indicating that recovery must be triggered.

The client re-sends the KeygenInit message and the encrypted recovery info. This allows Scalar Tss to reconstruct the Share KV Store by decrypting the recovery info using the party's mnemonic.