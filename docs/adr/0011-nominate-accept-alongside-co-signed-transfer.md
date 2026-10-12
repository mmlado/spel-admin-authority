# `admin_nominate` / `admin_accept` sit alongside the co-signed `admin_transfer`

**Status:** accepted.

`admin_transfer` moves the Admin in one transaction that the current Admin and a `Signer` candidate both sign. The witness exchange that produces it fixes every signer's nonce when the message is built, so the blob dies as soon as either key signs anything else (see Witness exchange in `CONTEXT.md`). A handover between two parties therefore has to keep both keys idle until submit.

We add a two-step handover on top of spel-authority's nomination (spel-authority ADR-0002) and keep `admin_transfer` unchanged:

- `admin_nominate(new_admin: AccountId)`: the current Admin signs alone. The nominee is stored in the Config PDA.
- `admin_accept()`: the nominee signs alone, and becomes Admin. A key nominee needs a program owner (`UnownedSigner`), checked here because accept is the first transaction the nominee's account appears in.
- A PDA nominee accepts through a chained call from its owning program, which passes the PDA's seeds so LEZ marks it authorized. That proves the owning program cooperates. The one-step `Pda` transfer only proves the PDA is deployed.
- Nominating again replaces the nominee, and nominating `AccountId::default()` cancels.

`admin_nominate` takes a bare `AccountId`. The glossary (`AdminCandidate`) and ADR-0005 option 3 avoid a bare id because a typo would install an Admin nobody controls. Here a typo only creates a nomination nobody can accept, the Admin does not move until the nominee signs, and nominating again fixes it. The avoid rule stays for one-step installs.

Renounce stays terminal: it clears the nomination, and only the Admin may nominate.

## Considered Options

**1. Keep both paths (chosen).** The co-signed transfer remains the single-transaction path that existing clients and the witness-exchange tooling use. Five injected instructions instead of three.

**2. Replace `admin_transfer` with nominate / accept.** Smaller surface and no nonce window anywhere. Rejected: removes a working path consumers already script against.

**3. Nominate / accept for keys only, one-step install for PDAs.** Rejected: the two paths would prove different things for PDAs, and only the chained-call accept proves the owning program takes part.

## Consequences

- Injected instructions grow from three to five. IDL pins and clients regenerate.
- `AdminConfig` grows from 32 to 64 bytes. Embedded consumers move every field after the slot by 32 bytes.
- freeze-authority's `wrap_instructions.exempt` list gains `admin_nominate` and `admin_accept`, so a frozen program can still hand its Admin over.
- A program that should hold the Admin as a PDA needs an instruction that chained-calls `admin_accept` with its seeds.
