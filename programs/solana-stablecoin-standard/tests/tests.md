Core Initialization & Setup Tests
| # | Test Case | Expected |
|---|-----------|----------|
| 1 | SSS-1 initialization succeeds | permanent_delegate = default, transfer_hook_program = None, preset = 0 |
| 2 | SSS-2 initialization succeeds | permanent_delegate = authority, blacklister = authority, preset = 1 |
| 3 | Re-initialize same PDA fails | AccountAlreadyInitialized error |
| 4 | Initialize with non-signing owner fails | MissingOwner error |
Role Management Tests
| # | Test Case | Expected |
|---|-----------|----------|
| 5 | update_minter succeeds when called by master_authority | minter role updated |
| 6 | update_minter fails when called by non-owner | Unauthorized error |
| 7 | update_freezer succeeds when called by master_authority | freezer role updated |
| 8 | update_freezer fails when called by non-owner | Unauthorized error |
| 9 | update_pauser succeeds when called by master_authority | pauser role updated |
| 10 | update_pauser fails when called by non-owner | Unauthorized error |
| 11 | update_blacklister succeeds (SSS-2 only) | blacklister role updated |
| 12 | update_blacklister fails in SSS-1 | NotCompliantMode error |
Mint / Burn Tests
| # | Test Case | Expected |
|---|-----------|----------|
| 13 | Mint succeeds when caller is minter | Tokens minted, TokensMinted event |
| 14 | Mint fails if caller is not minter | UnauthorizedMinter error |
| 15 | Mint fails when paused | MintPaused error |
| 16 | Mint exceeds supply_cap fails | Overflow error |
| 17 | Burn succeeds when caller is minter | Tokens burned, TokensBurned event |
| 18 | Burn fails if caller is not minter | UnauthorizedMinter error |
| 19 | Burn more than balance | InstructionError (SPL handles) |
Freeze / Thaw Tests
| # | Test Case | Expected |
|---|-----------|----------|
| 20 | freeze_account succeeds when caller is freezer | AccountFrozen event |
| 21 | freeze_account fails if caller is not freezer | UnauthorizedFreezer error |
| 22 | thaw_account succeeds when caller is freezer | AccountThawed event |
| 23 | thaw_account on non-frozen account | Succeeds (no-op) |
Blacklist Tests (SSS-2 only)
| # | Test Case | Expected |
|---|-----------|----------|
| 24 | blacklist_add succeeds when caller is blacklister | BlacklistEntry PDA created, AddedToBlacklist event |
| 25 | blacklist_add fails if already blacklisted | AlreadyBlacklisted error |
| 26 | blacklist_add fails in SSS-1 | NotCompliantMode error |
| 27 | blacklist_remove succeeds when caller is blacklister | BlacklistEntry PDA closed, RemovedFromBlacklist event |
| 28 | blacklist_remove fails if not blacklisted | NotBlacklisted error |
| 29 | blacklist_remove fails in SSS-1 | NotCompliantMode error |
Seize Tests (SSS-2 only)
| # | Test Case | Expected |
|---|-----------|----------|
| 30 | Seize succeeds when caller is permanent_delegate | Tokens transferred, TokensSeized event |
| 31 | Seize fails if caller is not permanent_delegate | UnauthorizedSeizer error |
| 32 | Seize fails if source account not blacklisted | NotBlacklisted error |
| 33 | Seize fails in SSS-1 | NotCompliantMode error |
Transfer Hook Tests
| # | Test Case | Expected |
|---|-----------|----------|
| 34 | Transfer succeeds when hook is None (SSS-1) | Transfer completes |
| 35 | Transfer succeeds when sender/receiver not blacklisted | Transfer completes |
| 36 | Transfer fails when sender is blacklisted | SenderBlacklisted error |
| 37 | Transfer fails when receiver is blacklisted | ReceiverBlacklisted error |
| 38 | Transfer fails when paused | TransfersPaused error |
Update Instructions Tests
| # | Test Case | Expected |
|---|-----------|----------|
| 39 | update_transfer_hook succeeds (SSS-2 only) | TransferHookUpdated event |
| 40 | update_transfer_hook fails in SSS-1 | NotCompliantMode error |
| 41 | update_paused succeeds when caller is pauser | PausedChanged event |
| 42 | update_paused fails if caller is not pauser | UnauthorizedPauser error |
Security / Edge Cases
| # | Test Case | Expected |
|---|-----------|----------|
| 43 | Overflow in mint amount | Overflow error |
| 44 | Overflow in seize amount | Overflow error |
| 45 | Invalid PDA seeds | Constraint error |
| 46 | Unauthorized signer on any instruction | Unauthorized error |
