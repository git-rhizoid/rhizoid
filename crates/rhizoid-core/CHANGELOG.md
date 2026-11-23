# Changelog

All notable changes to this project are documented here.
## [0.3.5]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])
- A tool names what its gate may change, committed with the pin ([#23](https://github.com/git-rhizoid/rhizoid/pull/23)) ([f99d220])
- An integration cutting tracks its releases source, and update pushes nothing for it ([#31](https://github.com/git-rhizoid/rhizoid/pull/31)) ([3a7ad52])
- A tool pin's key may have quoted parts, as a Cargo [patch."URL"] entry does ([#33](https://github.com/git-rhizoid/rhizoid/pull/33)) ([bd9e9ff])
- A tag origin moved is a tag-origin finding; a failed git call always says why ([#36](https://github.com/git-rhizoid/rhizoid/pull/36)) ([8963ccc])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])
- Rhizoid bump keeps every pin in one place and bumps tool pins on branches ([#22](https://github.com/git-rhizoid/rhizoid/pull/22)) ([de65654])
- Rhizoid follow lays a slice exactly as idealkb's follow.sh does ([#25](https://github.com/git-rhizoid/rhizoid/pull/25)) ([2daaf9a])
- Bump now moves slice pins through follow, on a branch, gated ([#27](https://github.com/git-rhizoid/rhizoid/pull/27)) ([7356a99])
- Rhizcut takes and keeps cuttings, recorded in the graph ([#29](https://github.com/git-rhizoid/rhizoid/pull/29)) **[BREAKING]** ([788dbd4])
- Cuttings release RELEASE-cut.N tags that bump pins, ordered by release then N ([#35](https://github.com/git-rhizoid/rhizoid/pull/35)) ([7358c36])
- Status reads a slice follower's own files as declared: a reason in [why] is current ([#38](https://github.com/git-rhizoid/rhizoid/pull/38)) ([5e0c0ae])
- Each pin's one mover is named on its edge (moved_by); bump now moves only its own ([#39](https://github.com/git-rhizoid/rhizoid/pull/39)) ([c29438b])
- One pin, one place: status names a second copy of a pin's tag name; bump refuses it ([#41](https://github.com/git-rhizoid/rhizoid/pull/41)) ([efc5331])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])
- Release v0.2.2 ([#18](https://github.com/git-rhizoid/rhizoid/pull/18)) ([a577486])
- Release v0.2.3 ([#21](https://github.com/git-rhizoid/rhizoid/pull/21)) ([bb2322b])
- Release v0.2.4 ([#24](https://github.com/git-rhizoid/rhizoid/pull/24)) ([514d7e7])
- Release v0.2.5 ([#26](https://github.com/git-rhizoid/rhizoid/pull/26)) ([606c795])
- Release v0.2.6 ([#28](https://github.com/git-rhizoid/rhizoid/pull/28)) ([9295128])
- Release v0.3.0 ([#30](https://github.com/git-rhizoid/rhizoid/pull/30)) ([d99ddaa])
- Release v0.3.1 ([#32](https://github.com/git-rhizoid/rhizoid/pull/32)) ([65721b8])
- Release v0.3.2 ([#34](https://github.com/git-rhizoid/rhizoid/pull/34)) ([49e671c])
- Release v0.3.3 ([#37](https://github.com/git-rhizoid/rhizoid/pull/37)) ([5420860])
- Release v0.3.4 ([#40](https://github.com/git-rhizoid/rhizoid/pull/40)) ([304411e])



## [0.3.4]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])
- A tool names what its gate may change, committed with the pin ([#23](https://github.com/git-rhizoid/rhizoid/pull/23)) ([f99d220])
- An integration cutting tracks its releases source, and update pushes nothing for it ([#31](https://github.com/git-rhizoid/rhizoid/pull/31)) ([3a7ad52])
- A tool pin's key may have quoted parts, as a Cargo [patch."URL"] entry does ([#33](https://github.com/git-rhizoid/rhizoid/pull/33)) ([bd9e9ff])
- A tag origin moved is a tag-origin finding; a failed git call always says why ([#36](https://github.com/git-rhizoid/rhizoid/pull/36)) ([8963ccc])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])
- Rhizoid bump keeps every pin in one place and bumps tool pins on branches ([#22](https://github.com/git-rhizoid/rhizoid/pull/22)) ([de65654])
- Rhizoid follow lays a slice exactly as idealkb's follow.sh does ([#25](https://github.com/git-rhizoid/rhizoid/pull/25)) ([2daaf9a])
- Bump now moves slice pins through follow, on a branch, gated ([#27](https://github.com/git-rhizoid/rhizoid/pull/27)) ([7356a99])
- Rhizcut takes and keeps cuttings, recorded in the graph ([#29](https://github.com/git-rhizoid/rhizoid/pull/29)) **[BREAKING]** ([788dbd4])
- Cuttings release RELEASE-cut.N tags that bump pins, ordered by release then N ([#35](https://github.com/git-rhizoid/rhizoid/pull/35)) ([7358c36])
- Status reads a slice follower's own files as declared: a reason in [why] is current ([#38](https://github.com/git-rhizoid/rhizoid/pull/38)) ([5e0c0ae])
- Each pin's one mover is named on its edge (moved_by); bump now moves only its own ([#39](https://github.com/git-rhizoid/rhizoid/pull/39)) ([c29438b])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])
- Release v0.2.2 ([#18](https://github.com/git-rhizoid/rhizoid/pull/18)) ([a577486])
- Release v0.2.3 ([#21](https://github.com/git-rhizoid/rhizoid/pull/21)) ([bb2322b])
- Release v0.2.4 ([#24](https://github.com/git-rhizoid/rhizoid/pull/24)) ([514d7e7])
- Release v0.2.5 ([#26](https://github.com/git-rhizoid/rhizoid/pull/26)) ([606c795])
- Release v0.2.6 ([#28](https://github.com/git-rhizoid/rhizoid/pull/28)) ([9295128])
- Release v0.3.0 ([#30](https://github.com/git-rhizoid/rhizoid/pull/30)) ([d99ddaa])
- Release v0.3.1 ([#32](https://github.com/git-rhizoid/rhizoid/pull/32)) ([65721b8])
- Release v0.3.2 ([#34](https://github.com/git-rhizoid/rhizoid/pull/34)) ([49e671c])
- Release v0.3.3 ([#37](https://github.com/git-rhizoid/rhizoid/pull/37)) ([5420860])



## [0.3.3]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])
- A tool names what its gate may change, committed with the pin ([#23](https://github.com/git-rhizoid/rhizoid/pull/23)) ([f99d220])
- An integration cutting tracks its releases source, and update pushes nothing for it ([#31](https://github.com/git-rhizoid/rhizoid/pull/31)) ([3a7ad52])
- A tool pin's key may have quoted parts, as a Cargo [patch."URL"] entry does ([#33](https://github.com/git-rhizoid/rhizoid/pull/33)) ([bd9e9ff])
- A tag origin moved is a tag-origin finding; a failed git call always says why ([#36](https://github.com/git-rhizoid/rhizoid/pull/36)) ([8963ccc])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])
- Rhizoid bump keeps every pin in one place and bumps tool pins on branches ([#22](https://github.com/git-rhizoid/rhizoid/pull/22)) ([de65654])
- Rhizoid follow lays a slice exactly as idealkb's follow.sh does ([#25](https://github.com/git-rhizoid/rhizoid/pull/25)) ([2daaf9a])
- Bump now moves slice pins through follow, on a branch, gated ([#27](https://github.com/git-rhizoid/rhizoid/pull/27)) ([7356a99])
- Rhizcut takes and keeps cuttings, recorded in the graph ([#29](https://github.com/git-rhizoid/rhizoid/pull/29)) **[BREAKING]** ([788dbd4])
- Cuttings release RELEASE-cut.N tags that bump pins, ordered by release then N ([#35](https://github.com/git-rhizoid/rhizoid/pull/35)) ([7358c36])
- Status reads a slice follower's own files as declared: a reason in [why] is current ([#38](https://github.com/git-rhizoid/rhizoid/pull/38)) ([5e0c0ae])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])
- Release v0.2.2 ([#18](https://github.com/git-rhizoid/rhizoid/pull/18)) ([a577486])
- Release v0.2.3 ([#21](https://github.com/git-rhizoid/rhizoid/pull/21)) ([bb2322b])
- Release v0.2.4 ([#24](https://github.com/git-rhizoid/rhizoid/pull/24)) ([514d7e7])
- Release v0.2.5 ([#26](https://github.com/git-rhizoid/rhizoid/pull/26)) ([606c795])
- Release v0.2.6 ([#28](https://github.com/git-rhizoid/rhizoid/pull/28)) ([9295128])
- Release v0.3.0 ([#30](https://github.com/git-rhizoid/rhizoid/pull/30)) ([d99ddaa])
- Release v0.3.1 ([#32](https://github.com/git-rhizoid/rhizoid/pull/32)) ([65721b8])
- Release v0.3.2 ([#34](https://github.com/git-rhizoid/rhizoid/pull/34)) ([49e671c])



## [0.3.2]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])
- A tool names what its gate may change, committed with the pin ([#23](https://github.com/git-rhizoid/rhizoid/pull/23)) ([f99d220])
- An integration cutting tracks its releases source, and update pushes nothing for it ([#31](https://github.com/git-rhizoid/rhizoid/pull/31)) ([3a7ad52])
- A tool pin's key may have quoted parts, as a Cargo [patch."URL"] entry does ([#33](https://github.com/git-rhizoid/rhizoid/pull/33)) ([bd9e9ff])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])
- Rhizoid bump keeps every pin in one place and bumps tool pins on branches ([#22](https://github.com/git-rhizoid/rhizoid/pull/22)) ([de65654])
- Rhizoid follow lays a slice exactly as idealkb's follow.sh does ([#25](https://github.com/git-rhizoid/rhizoid/pull/25)) ([2daaf9a])
- Bump now moves slice pins through follow, on a branch, gated ([#27](https://github.com/git-rhizoid/rhizoid/pull/27)) ([7356a99])
- Rhizcut takes and keeps cuttings, recorded in the graph ([#29](https://github.com/git-rhizoid/rhizoid/pull/29)) **[BREAKING]** ([788dbd4])
- Cuttings release RELEASE-cut.N tags that bump pins, ordered by release then N ([#35](https://github.com/git-rhizoid/rhizoid/pull/35)) ([7358c36])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])
- Release v0.2.2 ([#18](https://github.com/git-rhizoid/rhizoid/pull/18)) ([a577486])
- Release v0.2.3 ([#21](https://github.com/git-rhizoid/rhizoid/pull/21)) ([bb2322b])
- Release v0.2.4 ([#24](https://github.com/git-rhizoid/rhizoid/pull/24)) ([514d7e7])
- Release v0.2.5 ([#26](https://github.com/git-rhizoid/rhizoid/pull/26)) ([606c795])
- Release v0.2.6 ([#28](https://github.com/git-rhizoid/rhizoid/pull/28)) ([9295128])
- Release v0.3.0 ([#30](https://github.com/git-rhizoid/rhizoid/pull/30)) ([d99ddaa])
- Release v0.3.1 ([#32](https://github.com/git-rhizoid/rhizoid/pull/32)) ([65721b8])



## [0.3.1]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])
- A tool names what its gate may change, committed with the pin ([#23](https://github.com/git-rhizoid/rhizoid/pull/23)) ([f99d220])
- An integration cutting tracks its releases source, and update pushes nothing for it ([#31](https://github.com/git-rhizoid/rhizoid/pull/31)) ([3a7ad52])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])
- Rhizoid bump keeps every pin in one place and bumps tool pins on branches ([#22](https://github.com/git-rhizoid/rhizoid/pull/22)) ([de65654])
- Rhizoid follow lays a slice exactly as idealkb's follow.sh does ([#25](https://github.com/git-rhizoid/rhizoid/pull/25)) ([2daaf9a])
- Bump now moves slice pins through follow, on a branch, gated ([#27](https://github.com/git-rhizoid/rhizoid/pull/27)) ([7356a99])
- Rhizcut takes and keeps cuttings, recorded in the graph ([#29](https://github.com/git-rhizoid/rhizoid/pull/29)) **[BREAKING]** ([788dbd4])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])
- Release v0.2.2 ([#18](https://github.com/git-rhizoid/rhizoid/pull/18)) ([a577486])
- Release v0.2.3 ([#21](https://github.com/git-rhizoid/rhizoid/pull/21)) ([bb2322b])
- Release v0.2.4 ([#24](https://github.com/git-rhizoid/rhizoid/pull/24)) ([514d7e7])
- Release v0.2.5 ([#26](https://github.com/git-rhizoid/rhizoid/pull/26)) ([606c795])
- Release v0.2.6 ([#28](https://github.com/git-rhizoid/rhizoid/pull/28)) ([9295128])
- Release v0.3.0 ([#30](https://github.com/git-rhizoid/rhizoid/pull/30)) ([d99ddaa])



## [0.3.0]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])
- A tool names what its gate may change, committed with the pin ([#23](https://github.com/git-rhizoid/rhizoid/pull/23)) ([f99d220])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])
- Rhizoid bump keeps every pin in one place and bumps tool pins on branches ([#22](https://github.com/git-rhizoid/rhizoid/pull/22)) ([de65654])
- Rhizoid follow lays a slice exactly as idealkb's follow.sh does ([#25](https://github.com/git-rhizoid/rhizoid/pull/25)) ([2daaf9a])
- Bump now moves slice pins through follow, on a branch, gated ([#27](https://github.com/git-rhizoid/rhizoid/pull/27)) ([7356a99])
- Rhizcut takes and keeps cuttings, recorded in the graph ([#29](https://github.com/git-rhizoid/rhizoid/pull/29)) **[BREAKING]** ([788dbd4])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])
- Release v0.2.2 ([#18](https://github.com/git-rhizoid/rhizoid/pull/18)) ([a577486])
- Release v0.2.3 ([#21](https://github.com/git-rhizoid/rhizoid/pull/21)) ([bb2322b])
- Release v0.2.4 ([#24](https://github.com/git-rhizoid/rhizoid/pull/24)) ([514d7e7])
- Release v0.2.5 ([#26](https://github.com/git-rhizoid/rhizoid/pull/26)) ([606c795])
- Release v0.2.6 ([#28](https://github.com/git-rhizoid/rhizoid/pull/28)) ([9295128])



## [0.2.6]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])
- A tool names what its gate may change, committed with the pin ([#23](https://github.com/git-rhizoid/rhizoid/pull/23)) ([f99d220])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])
- Rhizoid bump keeps every pin in one place and bumps tool pins on branches ([#22](https://github.com/git-rhizoid/rhizoid/pull/22)) ([de65654])
- Rhizoid follow lays a slice exactly as idealkb's follow.sh does ([#25](https://github.com/git-rhizoid/rhizoid/pull/25)) ([2daaf9a])
- Bump now moves slice pins through follow, on a branch, gated ([#27](https://github.com/git-rhizoid/rhizoid/pull/27)) ([7356a99])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])
- Release v0.2.2 ([#18](https://github.com/git-rhizoid/rhizoid/pull/18)) ([a577486])
- Release v0.2.3 ([#21](https://github.com/git-rhizoid/rhizoid/pull/21)) ([bb2322b])
- Release v0.2.4 ([#24](https://github.com/git-rhizoid/rhizoid/pull/24)) ([514d7e7])
- Release v0.2.5 ([#26](https://github.com/git-rhizoid/rhizoid/pull/26)) ([606c795])



## [0.2.5]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])
- A tool names what its gate may change, committed with the pin ([#23](https://github.com/git-rhizoid/rhizoid/pull/23)) ([f99d220])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])
- Rhizoid bump keeps every pin in one place and bumps tool pins on branches ([#22](https://github.com/git-rhizoid/rhizoid/pull/22)) ([de65654])
- Rhizoid follow lays a slice exactly as idealkb's follow.sh does ([#25](https://github.com/git-rhizoid/rhizoid/pull/25)) ([2daaf9a])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])
- Release v0.2.2 ([#18](https://github.com/git-rhizoid/rhizoid/pull/18)) ([a577486])
- Release v0.2.3 ([#21](https://github.com/git-rhizoid/rhizoid/pull/21)) ([bb2322b])
- Release v0.2.4 ([#24](https://github.com/git-rhizoid/rhizoid/pull/24)) ([514d7e7])



## [0.2.4]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])
- A tool names what its gate may change, committed with the pin ([#23](https://github.com/git-rhizoid/rhizoid/pull/23)) ([f99d220])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])
- Rhizoid bump keeps every pin in one place and bumps tool pins on branches ([#22](https://github.com/git-rhizoid/rhizoid/pull/22)) ([de65654])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])
- Release v0.2.2 ([#18](https://github.com/git-rhizoid/rhizoid/pull/18)) ([a577486])
- Release v0.2.3 ([#21](https://github.com/git-rhizoid/rhizoid/pull/21)) ([bb2322b])



## [0.2.3]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])
- Release v0.2.2 ([#18](https://github.com/git-rhizoid/rhizoid/pull/18)) ([a577486])



## [0.2.2]


### Bug Fixes
- Status asks origin and the named releases source, not only the clone's own refs ([#17](https://github.com/git-rhizoid/rhizoid/pull/17)) ([5384f37])
- Send-back drafts: numbered past versions sent, replaced safely on a re-run ([#19](https://github.com/git-rhizoid/rhizoid/pull/19)) ([d485f9b])


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])
- Release v0.2.1 ([#16](https://github.com/git-rhizoid/rhizoid/pull/16)) ([3898cbe])



## [0.2.1]


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])
- Rhizoid send-back makes a branch a series version for its upstream ([#15](https://github.com/git-rhizoid/rhizoid/pull/15)) ([d7158a9])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])
- Release v0.2.0 ([#14](https://github.com/git-rhizoid/rhizoid/pull/14)) ([eb0d679])



## [0.2.0]


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])
- Rhizoid status reads integration edges; assemble ends worktrees, reuses unchanged tags, stays general ([#13](https://github.com/git-rhizoid/rhizoid/pull/13)) **[BREAKING]** ([bc3928d])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])
- Release v0.1.1 ([#10](https://github.com/git-rhizoid/rhizoid/pull/10)) ([dbe7a85])



## [0.1.1]


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])
- Rhizoid assemble merges a named list of branches onto its base and tags it ([#12](https://github.com/git-rhizoid/rhizoid/pull/12)) ([3e9efcb])


### Miscellaneous
- Release v0.1.0 ([#9](https://github.com/git-rhizoid/rhizoid/pull/9)) ([928f8e1])



## [0.1.0]


### CI/CD
- Release as git tags and GitHub releases, not through crates.io ([#8](https://github.com/git-rhizoid/rhizoid/pull/8)) ([f00a451])


### Features
- Implement the boundary layer (manifest, CLI, ports) as a two-crate workspace ([#1](https://github.com/git-rhizoid/rhizoid/pull/1)) ([5f39def])
- Rhizoid status reads every edge of a repository graph, and agrees with graph-drift.sh ([#7](https://github.com/git-rhizoid/rhizoid/pull/7)) ([39845af])


