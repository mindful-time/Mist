# Mist licensing and distribution

Checked 2026-10-02. This is a focused licensing assessment, not legal advice or
a complete transitive dependency/asset audit.

## Current position

Mist's own code is MIT: [LICENSE](../LICENSE) contains the MIT grant and
[Cargo.toml](../Cargo.toml) declares `license = "MIT"`. Opening the repository
does not itself change this license. Third-party components retain their
separate terms.

## MIT versus Apache-2.0

Both permit modification, redistribution, commercial use, and proprietary
derivatives; neither requires forks to publish their changes. Neither prevents
someone selling a competing fork. MIT chiefly requires keeping the copyright
and permission notice. It contains no explicit patent-license clause; this is
not a claim that every jurisdiction rejects implied rights.
([OSI MIT text](https://opensource.org/license/mit),
[Apache redistribution FAQ](https://www.apache.org/foundation/license-faq.html#Distribute-changes))

Apache-2.0 adds an explicit contributor patent grant, limited to claims
necessarily infringed by the contribution or its combination with the work.
That grant terminates for a recipient bringing specified patent litigation
against the work. Redistribution requires the license, prominent changed-file
notices, retained applicable source notices, and applicable attribution from an
upstream `NOTICE` when one exists. A `NOTICE` is not mandatory merely because a
project chooses Apache. Apache explicitly grants no trademark permission;
neither license substitutes for a separate brand/trademark policy.
([Apache license §§3–6](https://www.apache.org/licenses/LICENSE-2.0),
[Apache application guidance](https://www.apache.org/foundation/license-faq.html#Apply-My-Software))

Recommendation (inference): Apache-2.0 is a useful long-term choice if Mist
prioritizes explicit contributor patent terms; MIT is simpler and adequate for
permissive distribution. Apache is not universally “better” and is unnecessary
just to download/install Mist. Keep MIT unless the owner explicitly approves
the change and confirms authority over contributed code; dependencies do not
become relicensed by changing Mist's root license.

## Third-party evidence and release follow-up

- Vendored `kokoro-micro` 1.3.0 declares Apache-2.0 and retains its upstream
  [LICENSE](../third_party/kokoro-micro/LICENSE),
  [NOTICE](../third_party/kokoro-micro/NOTICE), and
  [patch provenance](../third_party/kokoro-micro/MIST_PATCH.md).
  [Misaki's official license](https://github.com/hexgrad/misaki/blob/main/LICENSE)
  is also Apache-2.0.
- Provisioning downloads model/voice artifacts from `8b-is/kokoro-tiny` revision
  `d9d564ee264fcd95459552767c8c713ed385c999`, not directly from Hugging Face.
  [That revision's license](https://github.com/8b-is/kokoro-tiny/blob/d9d564ee264fcd95459552767c8c713ed385c999/LICENSE)
  is Apache-2.0; the
  [official Kokoro-82M model card](https://huggingface.co/hexgrad/Kokoro-82M)
  identifies Apache-licensed weights. Record this conversion/artifact provenance
  alongside model notices; this check does not independently audit training data.
- Bundled Noto Sans Devanagari UI retains Google's copyright and SIL OFL 1.1
  at the [recorded upstream revision](https://github.com/google/fonts/blob/a559a6efcfed22bf50219f52ecefcf20b9522408/ofl/notosansdevanagariui/OFL.txt).
  Its own license must remain with the font; Mist need not become OFL.
- Fixed the identified MIT notice gap: added complete copyright/permission
  texts for incorporated Cutlet, python-pinyin, and pinyin-to-ipa material in
  [THIRD_PARTY_LICENSES](../third_party/kokoro-micro/THIRD_PARTY_LICENSES), using
  recorded upstream license-file revisions predating the vendored snapshot.
  The file distinguishes these verified notice revisions from unknown original
  table-input revisions: upstream regeneration instructions use floating URLs.
  Cargo packaging resources include it as
  `licenses/kokoro-micro-THIRD_PARTY_LICENSES`, alongside Mist's license,
  Kokoro Micro's Apache license/NOTICE, and the Noto OFL. This verifies the
  resource configuration, not every built installer or all transitive notices.
