# Disclaimers and names pitwall may show

Research for the pitwall wayfinder ticket "What disclaimers and names may pitwall show?" ([#22](https://github.com/Vankata03/pitwall/issues/22)).
Researched 2026-10-31. Quotes are verbatim from the fetched page unless marked otherwise.

**This is not legal advice.** It reports what the published terms say and separates that from interpretation. The published terms are the rights holders' own policies, not the law, and they can change without notice. Someone qualified should review the plan before pitwall goes public, particularly the names and the project name.

## Summary

- **Two published texts ask for a disclaimer.** F1 says an unofficial fan website "should contain" a set wording in the footer of its landing page. EA says a project displaying its game content must "include the following statement". pitwall should show both.
- **A disclaimer does not grant rights.** It keeps pitwall inside the conditions of F1's and EA's fan tolerance, and it licenses nothing.
- **Arriving in telemetry does not make a name licensed.** EA says: "We do not provide you with any permission to use third-party content from our games", and it lists talent and brands as examples. F1 sends team and driver rights to the teams and drivers. No source checked gives permission to show team or driver names. Plain-text naming is the same thing every comparable tool does, and it has an arguable basis in trademark law, but that is interpretation.
- **Team names do not actually arrive in the telemetry.** The packets carry a numeric `m_teamId`. The names are in EA's specification appendix, so pitwall must ship its own lookup table. Driver names do arrive, in `m_name`, except that in multiplayer they can be other people's Steam IDs or LAN names.
- **F1's logos are off limits and F1's word marks cannot brand pitwall.** Logos need a written licence "for any purpose or in any medium". Word marks cannot be part of a product name, and in relation to apps they may appear only in accompanying descriptive text.
- **The name `pitwall` contains no listed F1 mark, but it is crowded.** PITWALL ONE, an unreleased Windows companion for F1 25 on Steam, is the closest of several products using the word. Trademark registers were not searched.

## Sources

| Key | Source | Status |
|---|---|---|
| [F1-G] | [F1 Guidelines](https://www.formula1.com/en/toolbar/guidelines.html), "Guidelines for the use of trade marks and intellectual property rights belonging to the Formula 1 companies". Event Titles dated "as of October 2026" | Fetched in full |
| [F1-LN] | [F1 Legal Notices](https://www.formula1.com/en/toolbar/legal-notices.html) | Fetched |
| [EA-CP] | [EA's content policy](https://help.ea.com/en/articles/security-and-rules/ea-content-policy/), "Updated 2 months ago" | Fetched in full |
| [EA-UA] | [EA User Agreement](https://tos.ea.com/legalapp/WEBTERMS/US/en/PC/), last updated May 14, 2026 | Fetched |
| [EA-GD] | [F1 25 game disclaimers](https://www.ea.com/games/f1/f1-25/game-disclaimers) | Fetched |
| [UDP-25] | "Data Output from F1® 25 Game" v3. [Official page](https://forums.ea.com/blog/f1-games-game-info-hub-en/f1%c2%ae-25-udp-specification/12187347) returned a bot challenge. [Verbatim mirror](https://github.com/MatMridul/APX-IQ/blob/main/docs/Data%20Output%20from%20F1%2025%20v3.txt) used | Mirror only |
| [UDP-26] | "Data Output from F1® 26 Game". [Verbatim mirror](https://github.com/MacManley/f1-26-udp/blob/main/README.md) | Mirror only |
| [PNG] | [pits-n-giggles repository](https://github.com/ashwin-nat/pits-n-giggles) at commit `3490db9`, and [its website](https://www.pitsngiggles.com) | Repository cloned and searched; website home page only |
| [P1] | [P1 Telemetry on the App Store](https://apps.apple.com/app/id6452589420) | Fetched |
| [SRT] | [Sim Racing Telemetry add-on on Steam](https://store.steampowered.com/app/3692310/) | Fetched |
| [PW1] | [PITWALL ONE on Steam](https://store.steampowered.com/app/5055450/) | Fetched |
| [EUTMR] | [Article 14, Regulation (EU) 2017/1001](https://gesetze.legal/eu/vo_eu_2017_1001/14), German text on a reproduction site | Not read on EUR-Lex |
| [AUDI] | CJEU C-334/22 *Audi v GQ*, via [Fieldfisher](https://www.fieldfisher.com/en/services/intellectual-property/intellectual-property-blog/running-rings-around-referential-use-of-trade-marks-the-cjeus-decision-in-audi-v-gq) and [Clifford Chance](https://cliffordchance.com/expertise/services/intellectual-property/global-ip-updates/2024/q2/the-court-of-justice-of-the-european-union-sheds-some-light-on-the-interpretation-of-the-referential-use-of-a-trademark.html) | Search summaries of law-firm notes; judgment not read |

## Verified: what the sources say

### Formula 1

- **Default rule.** "As a rule our trade marks cannot be used by third parties without a specific written licence from us. However certain trade marks can be used editorially and by fans without requiring a license or permission from us where these Guidelines are adhered to." [F1-G]
- **Who counts as a fan.** Fans are those "active in supporting the races but without doing so in a materially commercial manner, subject to our discretion". [F1-G]
- **Discretion.** "As it is impossible to provide guidance on all possible uses, we reserve our right to take action against unauthorised uses which we consider to infringe our rights." [F1-G]
- **Permitted Word Marks.** F1, Formula 1, Formula One, FIA Formula One World Championship, Grand Prix, Paddock Club, Formula One Paddock Club, F1 Paddock Club, plus the listed Event Titles. "[W]e allow the Permitted Word Marks to be used to inform, not to brand." The list of marks "is not a full list". [F1-G]
- **Logos.** "You may not use, for any purpose or in any medium, any of our Logos unless you have an express written licence from the Formula 1 companies." [F1-G]
- **Fundamental Principles**, which apply "at all times". You must "be clear that what you are doing is not official, approved or endorsed by us or associated or connected with us"; "not use our Logos"; and "not integrate Our Marks into any other marks and sufficiently differentiate the use of Our Marks from other marks". [F1-G]
- **Disclaimer.** "If the website is dedicated to the FIA FORMULA ONE WORLD CHAMPIONSHIP and is unofficial, such as Fan use, the website should contain the following disclaimer: This website is unofficial and is not associated in any way with the Formula 1 companies. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX and related marks are trade marks of Formula One Licensing B.V." It "should be inserted in the footer of the landing and/or home page of the website." [F1-G]
- **Apps.** "Our Permitted Word Marks can be used editorially for non-commercial purposes in relation to apps so long as the use is to inform or report and not to brand, i.e. in the paragraphs of accompanying text to describe the app. Other Intellectual Property Rights cannot be used in apps." [F1-G]
- **Other Intellectual Property Rights** cover results and timing data, the official typeface, statistics, still images, audio and audio-visual content, official artworks, graphics, assets and textures, and written content. [F1-G]
- **Names of products.** "Our Marks cannot be used or registered as part of a company name, trade name, product name, service name, social media name and/or handle nor can they be used or registered in any trade mark or service mark." [F1-G]
- **Variations.** "Our Marks, including variants, misspellings and derivatives, cannot be incorporated into logos, composite logos, website icons, or used to create a brand identity." [F1-G]
- **Domains.** Word marks "cannot be incorporated into or registered as domain names for commercialised websites". Non-commercial use "may be tolerated at our discretion". A mark may follow the domain as a path but not as a prefix sub-domain. [F1-G]
- **Appearance.** Word Marks go in Title Case with ™ or in BLOCK CAPITALS; in body copy the ™ "is not mandatory". They "should not be stylised or written in a font style that is different to the surrounding text". [F1-G]
- **Grammar.** They are "attributive modifiers", as in "F1 racing"; they "should not be used as nouns" and "should never be used in the possessive form". [F1-G]
- **Fonts.** Header typefaces "cannot be used under any circumstances without an express written license". Titillium is open source, but "should not be used in any manner to create an unauthorised association". [F1-G]
- **Simulators.** "Motorsport simulators and/or software that simulates auto racing … should not make any use of the FORMULA 1 Rights without an express written license." Third-party names used with simulators "are the property of their respective owners". [F1-G]
- **Sim racing and esports.** "The FORMULA 1 Rights cannot be used in connection with any sim racing and/or esports team – including branding of any unofficial sim racing and/or esports team or league - without an express written license." [F1-G]
- **Games.** "Our Permitted Word Marks cannot be used to brand any game." [F1-G]
- **Commercial use.** "The FORMULA 1 Rights cannot be used on or in relation to any goods/services provided for sale/promotional giveaways without an express licence". [F1-G]
- **Endorsement.** The rights "cannot be used in conjunction with an unauthorised third party's brand or logo." [F1-G]
- **Teams, drivers and circuits.** Team and driver licensing goes to "the respective teams and/or drivers' management teams directly". For "circuit outlines", "please contact the owner of the IP directly". The guidelines are otherwise silent on team and driver names. [F1-G]
- **Video.** Word marks may be used in non-commercial videos "to inform and report and not to brand", and should not be the first words of a video's title in the style "The F1 Vlog: …". [F1-G]
- **Owner of the marks.** Formula One Licensing BV. [F1-LN; F1-G]

### EA

- **Fan permission.** "Our fans can use our game content, including gameplay and original characters, for personal uses or personal projects, including fan sites and videos, as long as they follow these principles". [EA-CP]
- **Required statement.** "Don't use our brands, studios, or games to make it seem like your project is endorsed by or affiliated with us including, for example, by using our logos or game art on your website. If you have a website or other location where you're displaying our game content, include the following statement: 'This [project/website] is not endorsed by or affiliated with EA or its licensors.'" [EA-CP]
- **No commercial use.** No selling and no paywalls "like Patreon". Passive advertising and partner-programme video monetisation are the exception. [EA-CP]
- **Branding.** "Our company, studio and game logos should not be combined with your own branding", nor may trademarks appear in a "profile picture, social handle, or domain name". [EA-CP]
- **Original work.** "Don't use our games to create knock-off or spin-off games or products." "Don't combine our game content with other third-party products, services, or brands." [EA-CP]
- **Licensed content.** "Many of our games include content that we license from third parties or that may otherwise have usage restrictions (for example, talent, brands, voices, music). Those third parties may have their own guidelines for how you can use their content. We do not provide you with any permission to use third-party content from our games. You use our game content at your own risk." [EA-CP]
- **Demos.** The rights-clearance form covers "an EA business partner" using games "at industry events or for product demos". A fan project is not an EA business partner, and EA says "Don't interpret our lack of response as an approval." [EA-CP]
- **User Agreement.** "You may not reverse engineer or attempt to extract or otherwise use source code or other data from EA Services, unless expressly authorized by EA or permitted by law." [EA-UA]
- **The game's own notice** shows that the F1 marks reach EA by licence: "F1® 2025 Game - an official product of the FIA FORMULA ONE WORLD CHAMPIONSHIP. © 2026 Electronic Arts Inc. EA, EA SPORTS, the EA SPORTS logo, and Codemasters are trademarks of Electronic Arts Inc." The F2 marks belong to the FIA and are "used exclusively under license". [EA-GD]

### The UDP specification

- **Terms.** The text states no terms of use, no permission for third-party tools and no attribution requirement. It carries one disclaimer: "This information is being provided under license from EA for reference purposes only and we do not make any representations or warranties about the accuracy or reliability of the information for any specific purpose." The licence itself is not named or linked. [UDP-25]
- **Legal Notice.** It is the game's trademark notice, not a set of conditions for developers. The 2026 document repeats it with a 2026 date. No "DISCLAIMER" line was found in the 2026 mirror. [UDP-25; UDP-26]
- **Team IDs.** The team is a number, `m_teamId`, mapped to names such as "Mercedes" and "Audi '26" in an appendix. The participant struct has no team-name string. [UDP-25; UDP-26]
- **Participant names.** "If the vehicle is controlled by AI, then the name will be the driver name. If this is a multiplayer game, the names will be the Steam Id on PC, or the LAN name if appropriate." On Xbox "the names will always be the driver name"; on PlayStation it is "the LAN name if playing a LAN game, otherwise it will be the driver name." [UDP-25]
- **Online-names setting.** `m_showOnlineNames` is defined as "The player's show online names setting, 0 = off, 1 = on". The text does not say what `m_name` contains when it is off. [UDP-25]
- **Driver IDs.** `m_driverId` is "255 if network human"; otherwise an appendix maps it to real names. [UDP-25]
- **Track names.** The Track IDs appendix uses place names such as "Melbourne", "Silverstone" and "Spa", not event titles. [UDP-25]

### Comparable tools

Their practice shows what is common. It is not evidence of permission.

| Tool | Disclaimer wording | Names and logos |
|---|---|---|
| **pits-n-giggles** [PNG] | None found: no match for "unofficial", "affiliat", "endorsed" or "trademark" in the repository, and no such wording on the home page. The README's only licence wording is MIT | Ships team logos as `assets/team-logos/*.svg`, and a team-name table in `lib/f1_types/team_id.py`. Description: "F1 23/24/25 Live Telemetry Tool" |
| **P1 Telemetry** [P1] | "P1 Telemetry is an unofficial app and is not associated in any way with the Formula 1 companies. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX and related marks are trade marks of Formula One Licensing BV." | Not reviewed |
| **Sim Racing Telemetry** add-on [SRT] | "Note: this product is not made by, or affiliated with, the developer and/or owners of the game." | Not reviewed |
| **PITWALL ONE** [PW1] | "PITWALL ONE is an independent third-party application and is not affiliated with or endorsed by Electronic Arts, Codemasters, Formula One Licensing B.V., Studio 397, Motorsport Games, KUNOS Simulazioni, 505 Games, the ACO, or their respective partners. Game titles and trademarks belong to their respective owners. Games sold separately." | Names the game as "EA SPORTS™ F1® 25" |

P1 Telemetry shows an adaptation of F1's website wording to an app by swapping the subject. PITWALL ONE names rights holders individually, which is more complete but goes out of date as they change.

### Name collisions for "pitwall"

These come from one web search, with PITWALL ONE and the iOS app opened directly. No trademark register was searched.

- **PITWALL ONE** by PixelDrift: a Windows companion for "EA SPORTS™ F1® 25", Le Mans Ultimate and the Assetto Corsa family. Release date "Coming soon"; it mentions a "Second Screen" and a track map "learned from your own laps". [PW1] It overlaps pitwall's audience and its description.
- [Pitwall: F1 Companion](https://apps.apple.com/app/id6762566510), an iOS app.
- [Pitwall F1](https://www.producthunt.com/p/pitwall-f1-macos-app/pitwall-f1), a macOS menu-bar timing app (from search results).
- [f1pitwall](https://pypi.org/project/f1pitwall/1.1.1/) on PyPI and [lmu-pitwall](https://github.com/Swizzjack/lmu-pitwall) on GitHub (from search results).

## Interpretation

Everything in this section is reasoning from the sources above, not something they state.

- **pitwall's category.** Its closest F1 heading is "Apps", and it also fits "website", because the collector serves a web page. It reads a game and simulates nothing, so the "Simulators" clause reads as aimed elsewhere, though its "software that simulates auto racing" wording leaves room for F1 to disagree.
- **Disclaimer strength.** F1's "should contain" is written for websites, and the Fundamental Principle underneath it ("must … be clear") is mandatory. EA's "include" applies to displaying "game content". Whether live telemetry is "game content" is not stated. Showing both costs one line, so the question need not be settled.
- **Plain names on the dashboard.** No source licenses them. EU trademark law limits a trademark owner's rights over use that identifies or refers to goods or services as the proprietor's, subject to honest practices [EUTMR Art. 14(1)(c), 14(2)]. *Audi v GQ* is reported to confine that exception, holding that faithfully reproducing the proprietor's product falls outside it [AUDI]. A name in plain text that says which team a car belongs to sits far from that case; a logo or a replica sits closer. Whether Art. 14 covers a free fan project at all is an open question: it concerns use "in the course of trade". Rights in a driver's name and likeness come from national personality law, which varies and was not researched.
- **Practical risk.** It rises with how far pitwall looks official. It rises again with commercial use and with visible use of real art on streams. The fan tolerance in both policies is discretionary and can end.
- **F1 Timing Data.** The Other Intellectual Property Rights include F1's own results and timing data. pitwall shows game data, and real-world data is out of scope, so this clause should not apply unless that scope changes.
- **Circuit outlines.** F1 treats these as the circuit owners' IP. The track-map decision ([#4](https://github.com/Vankata03/pitwall/issues/4)) ships maps built from pitwall's own recordings. Those are original files, but they depict the same shapes. No source addresses whether that matters. Maps learned locally by the user raise the question far less.
- **The specification.** Its "reference purposes only" wording is a warranty disclaimer rather than a licence for developers. Reading a documented game output is not obviously the reverse engineering or extraction the User Agreement bars, but EA's documents never say "expressly authorized".

## Recommendation

### Disclaimer text

The full notice goes on the collector's status/about page, in the README and on every release page:

> pitwall is unofficial and is not associated in any way with the Formula 1 companies. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX and related marks are trade marks of Formula One Licensing B.V.
>
> This project is not endorsed by or affiliated with EA or its licensors.
>
> Team names, driver names and other third-party names and marks belong to their respective owners and are used only to identify them.

The first paragraph is F1's wording with the subject changed. The second is EA's wording with `project` chosen. The third is pitwall's own and no source requires it. Keep the first two as they are. The trademark sentence stays verbatim even though it is clumsy, because the point of using it is that it is F1's.

Because stream viewers and demo audiences never open an about page, the dashboard itself carries one short line in its chrome, on every layout and preset, at the 11 px minimum, linking to the full notice:

> Unofficial. Not associated with the Formula 1 companies or EA.

The wording of this short line is pitwall's own. Published videos and demo pages carry the full notice in their description.

### Names

- **Driver names:** show `m_name` as received, in plain text and the dashboard's own font. Ship no driver table, portraits, signatures or helmets. Three-letter codes derived from `m_name` are fine.
- **Multiplayer names:** treat these as personal data, because a Steam ID on a public stream is another person's identifier. Show a human whose `m_showOnlineNames` is `0` by race number. Check what the game sends in that case during the build.
- **Team names:** ship one lookup table of the short names in EA's appendix, such as "Mercedes" and "McLaren". Leave out title sponsors' names. Label the player's custom team "My Team".
- **Team identity:** use game livery colours only, as already decided in [#12](https://github.com/Vankata03/pitwall/issues/12). No team logos, even though pits-n-giggles ships them.
- **Tracks and sessions:** use place names from EA's appendix ("Spa") and plain session names ("Race"). Never use Event Titles or "Grand Prix" as UI labels. Event Titles also carry sponsors' names.
- **Recordings:** a `.pitrec` holds raw names, so sharing one shares them. Say so wherever sharing is documented.

### Logos and art

- Use no F1, FIA, EA, EA SPORTS, Codemasters, team, sponsor or tyre-supplier logos anywhere: UI, favicon, README, screenshots, tiles or release pages.
- Keep real art in local livery packs only, as [#12](https://github.com/Vankata03/pitwall/issues/12) decided. The disclaimer does not cover packs.
- README images, store-style screenshots and official demo footage use the fallback car. A demo with a pack is the owner's risk under the earlier findings.
- Use no F1 header typeface. Oxanium stays. If Titillium were ever considered, avoid it: F1 names it specifically.

### The project and its marks

- Keep F1, Formula 1, Grand Prix, EA and game titles out of the project name, binary name, package names, domains, social handles, logo, preset names and tile names.
- Do not name McLaren or Splunk in the "Pitwall Classic" preset or its description in the UI.
- Refer to the game descriptively, once, in running text: "a telemetry dashboard for the F1® 25 game". The current repository description, "for F1 game telemetry", follows the attributive rule.
- Never write "F1's" or use F1 as a noun in project copy, and never style the marks differently from the surrounding text. Avoid "official", "licensed" and "F1 dashboard" as a title.
- Video titles and social posts must not open with an F1 mark.
- Keep pitwall free: no paid tier, paywall or sponsor branding beside the marks. Any change there reopens this ticket. Ask before lending the name to a sim-racing team or league, because F1 requires a licence there.

## Not verified

- EA's official specification page was not readable; both specification texts come from community mirrors.
- `m_name` behaviour when `m_showOnlineNames` is `0`.
- Trademark registers (EUIPO, USPTO, WIPO) for "pitwall" or "pit wall", and whether F1 claims either. F1's list of marks is "not a full list".
- Individual teams' name policies. Earlier research covers McLaren, Mercedes and Ferrari website terms, not name use.
- Personality-rights law for drivers, in any country.
- Any judgment or statute beyond the Art. 14 text and law-firm summaries noted above.
- pits-n-giggles' website beyond the home page, and its in-app screens.

Written confirmation from brandprotection@f1.com is the one route that would settle F1's position. F1 invites queries there. Asking also puts the project in front of them, which is a judgement call for the owner.
