# Source attribution

Source: [anreonyr/sqware](https://github.com/anreonyr/sqware).
Source checkpoints: `89ecffd0f8ce2077cd757d9561be141c87b8ec3b` and `293c15fc97a9d9a4295810d10fe804e4cdec63c4`.

Historical Hub API, client, runtime and host fixtures. Their original source
and detailed extraction records remain in Git history. The native v2 Machine,
Weave/Twine boundary and package-owned tests replace that runtime contract.

Internal PCI configuration/BAR mechanisms also draw on
[shuyra-arche/driver-pci](https://github.com/shuyra-arche/driver-pci/tree/69d43c9018ce6c35f63a83648050e33fe815504c),
especially its ECAM geometry, exact BAR windows and partial-effect retention
requirements. No old SDK capability implementation, user IOMMU or PLIC backend
is copied into the native Hub. The new mechanism requires separately validated
platform authorization and retirement; its Host models are not hardware proof.

Source-recorded author identities:

- `anreonyr <anreonyr@gmail.com>`
- `anreonyr <88045926+anreonyr@users.noreply.github.com>`
- `Serial Device <anreonyr@gmail.com>`

The original source checkpoint contains no project license grant. This repository
preserves source headers and attribution and does not infer a license from publication.
Third-party dependencies retain their own upstream licenses. Original history and
previous detailed extraction records remain in Git history; no history is rewritten.
