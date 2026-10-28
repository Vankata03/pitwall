# Real-livery car art: sources and terms

Research for the pitwall wayfinder ticket "Where can real-livery car art come from, and what may we do with it?"
Researched 2026-10-29. Every claim below cites the page it came from. Quotes are verbatim from the fetched page.

**This is not legal advice.** It sets out what the published terms say, flags anything that could not be verified, and draws no legal conclusions beyond the wording of those terms. Before any real livery is shown in a public demo or shipped in a build, someone qualified should check the plan, or the rights holders should give written permission.

## Summary

- **No public source licenses real 2025 F1 livery art for use in an app.** None of the team sites, the F1 site, EA or the 3D marketplaces checked grants it. Their terms fall into three groups: personal non-commercial use of the site content only (F1, McLaren, Mercedes, Ferrari); editorial or journalistic use only (team media portals, Editorial-licence 3D models); or no permission at all for third-party licensed content (EA, which covers liveries because they are team and sponsor IP).
- **F1's brand guidelines say outright that "Other Intellectual Property Rights cannot be used in apps".** Those rights include F1's official artwork, graphics, assets, textures, still images and official typeface. F1 says licences for team or driver rights must come from the teams themselves.
- **Redistribution (use case d) is the strictest case.** Every source either forbids it or requires the art to be locked in a proprietary format that end users cannot extract. An open-source repository cannot meet that condition.
- **Splunk most likely used the McLaren render under its contracted rights as McLaren's Official Technology Partner.** McLaren runs a dedicated media and asset portal on Brandfolder. Partner deals are the normal route for this kind of use, and pitwall has no such deal. The exact source of the render was **not** confirmed.
- **The architecture that keeps pitwall free of redistributed trademarked art:** pitwall ships only an original, stylised car drawing filled in team colours, plus an asset-pack loader that reads user-supplied files from a local folder. pitwall's repository, binaries and documentation should not bundle, download or point to real-livery files.

## Source × use-case table

Legend: **Yes** = the terms allow it in so many words. **No** = the terms forbid it in so many words, or grant nothing that would cover it. **Unclear** = the terms do not address the case, or two sources conflict. "Live demo" means use case (c), public demos and presentations.

| Source | (a) Private personal use | (b) Livestream | (c) Public demo / presentation | (d) Redistribute in app (OSS repo or binary) |
|---|---|---|---|---|
| **formula1.com content** (images, artwork) | Yes, for "personal, non-commercial use", and copies must keep their notices intact [F1-LN] | Unclear. Fans may make "non-commercial videos that inform or report, not brand"; still images "should not be reproduced without the … rights holder's consent" [F1-G] | Unclear, leaning No: "materially commercial" use is excluded, and other uses need consent [F1-G] | **No**: "Other Intellectual Property Rights cannot be used in apps" [F1-G] |
| **F1 logos and word marks** | Logos are never allowed without an express licence [F1-G] | Word marks may be used only to inform, not to brand; logos No [F1-G] | Same as (b) | Logos No; word marks may appear only in descriptive text about the app [F1-G] |
| **Team public websites** (McLaren, Mercedes, Ferrari) | Yes, within the site licence. McLaren: personal, non-commercial use, unmodified, and graphics may not be used apart from their text [MCL-ToU 2.1]. Mercedes: "personal use only" [MGP-LN]. Ferrari: "personal, educational or instructional purposes", no commercial purpose, no modification [FER-L] | No grant. McLaren 2.4: any other use "is prohibited" | No grant | **No**. McLaren 2.5: no part may be stored in any "public or private electronic retrieval system" [MCL-ToU]. Mercedes: no commercial use and no reposting on other sites [MGP-LN] |
| **Team media portals / press kits** (McLaren on Brandfolder, Mercedes MARS, Aston Martin on Canto, Red Bull Content Pool) | Registration or press accreditation usually required. Mercedes media terms limit use to "end-users", one-time use; "No commercial use of any kind" without written consent [MGP-GTC §1 Par.7, §3] | Unclear. The terms are written for journalists and editorial use [MGP-GTC; RBCP] | Unclear, leaning No | **No**. Mercedes: rights cannot be assigned to third parties, and any use "over and above one-time use" needs consent [MGP-GTC §3 Par.4, 7]. McLaren and Red Bull portal terms could not be read (JavaScript-only portals) |
| **EA F1 25 game assets** (in-game screenshots and footage) | The content policy allows personal use, but EA gives "[no] permission to use third-party content from our games", and liveries are licensed team and sponsor brands [EA-CP] | Game footage and screenshots in streams: Yes, including partner-programme monetisation [EA-CP]. Real liveries carry no EA permission and are used "at your own risk" [EA-CP] | Commercial use No; "product demos" need a rights-clearance request, which is offered to EA partners [EA-CP] | **No**. No distributing game content; no "data mining" of assets; the User Agreement bans extracting data or reverse engineering [EA-CP, EA-UA §2] |
| **EA game files** (models ripped from the install) | **No**: "obtain assets through 'data mining'" is prohibited [EA-CP]; "You may not reverse engineer or attempt to extract … data from EA Services" [EA-UA §2] | No | No | No |
| **TurboSquid** (Editorial Use Only models; real F1 cars carry this label) | Not listed. Permitted uses are news, documentary and academic work [TS-Ed] | No: "may not be used for any commercial, promotional, advertising or merchandising use"; Depicted IP use "limited to news reporting" [TS-Lic] | No (trade-show and exhibit displays are listed as prohibited) [TS-Ed] | **No**: "You may NOT re-distribute, publish, or make 3D Models available to any third party"; models in public software must be in "proprietary formats so that they cannot be opened or imported" [TS-Lic] |
| **CGTrader** (Editorial licence) | The help article says personal projects with no plan to commercialise are allowed; the T&C say only "legitimate, editorial purposes". The two conflict [CGT-T&C §22.2; CGT-help via search] | Unclear | Unclear, leaning No | **No**: Royalty Free models may be used only as an "Incorporated Product" that the end user cannot extract; "resale or redistribution … is expressly prohibited" (§21A.1, .2, .6). Editorial products "may contain material that is not released from its rights holders" [CGT-T&C] |
| **Sketchfab** | The Store has closed. Only free downloads remain [SKF-help] | Depends on the model's CC licence | Depends on the model's CC licence | CC licences can allow redistribution of the model's copyright, but under CC BY 4.0 §2(b)(2) "Patent and trademark rights are not licensed" [CC-BY4]. Under the old Store's Standard licence, branded content was not allowed [SKF via search] |
| **Fab** (Epic) | Fab has a Standard licence (Personal or Professional tier) and CC-BY [FAB-doc] | The EULA could not be read (HTTP 403) | Same | Same; open question |
| **Wikimedia Commons photos** (e.g. MCL39 race photos, usually CC BY-SA) | Yes under the CC licence | Yes under the CC licence, with attribution | Same | Copyright side: allowed under CC BY-SA if attributed, and share-alike applies to derivatives such as cut-outs. Trademark side: the Commons "Trademarked" tag says "you have to ensure that you have the legal right" [WMC-TM; CC-BY4] |
| **Fan renders** (ArtStation, DeviantArt, Behance, and so on) | Needs the artist's permission (the artist holds copyright in the render). Team and sponsor marks are still not licensed | Same | Same | Same. The platform terms were **not** researched |
| **Hot-linking from team or F1 servers** (the app fetches images at runtime) | Same as the site terms above | Same | Same | Avoids shipping the files, but F1 says Other IP "cannot be used in apps", and McLaren 2.5 bars "electronic retrieval system[s]" [F1-G; MCL-ToU] |
| **Written licence from F1 and/or the team** | The only route that grants (b) to (d) explicitly. F1 asks for "a full proposal including financial terms" at brandprotection@f1.com and sends team-rights requests to the teams [F1-G] | Yes, if licensed | Yes, if licensed | Yes, if licensed |

## Notes per source

### Formula 1 (Formula One Licensing BV / Formula One Digital Media)

- **Trademark owner.** "The F1 logos, F1 FORMULA 1 logos, F1 FIA FORMULA 1 WORLD CHAMPIONSHIP logo, FORMULA 1, FORMULA ONE, F1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX, F1 GRAND PRIX and related marks are trademarks of Formula One Licensing BV, a Formula 1 company." [F1-LN]
- **Site content** is "for your personal, non-commercial use only, save where expressly provided". Users must not "modify, copy, reproduce, republish, upload, frame, post, transmit or distribute" material without prior written consent. [F1-LN]
- **F1 brand guidelines** [F1-G] (the main F1 source for pitwall):
  - **Other Intellectual Property Rights** include "Results and Timing data; Official Typeface; Statistics; Still Images; Audio and Audio Visual Content; Official Artworks, Graphics, Assets and Textures; Written Content".
  - **Apps:** "Our Permitted Word Marks can be used editorially for non-commercial purposes in relation to apps so long as the use is to inform or report and not to brand i.e. in the paragraphs of accompanying text to describe the app. Other Intellectual Property Rights cannot be used in apps."
  - **Simulators:** "Motorsport simulators and/or software that simulates auto racing … should not make any use of the FORMULA 1 Rights without an express written license." pitwall is a telemetry dashboard, not a simulator. Whether F1 would treat it as "software … in association with" one is an open question; the guidelines do not say.
  - **Teams:** "If you would like to enquire about licensing any of the rights of the teams or drivers please contact the respective teams and/or drivers' management teams directly." The guidelines say nothing about team names, cars or liveries; they leave those rights to the teams.
  - **Still images:** "All rights in still images/ photos/ screenshots are the property of their respective owners which in certain cases may be the Formula 1 companies."
  - **Circuit outlines** are listed as circuit-owner IP: "contact the owner of the IP directly". This is outside this ticket, but it matters for any track-map tile.
  - **AI and data mining:** F1 Rights may not be used for text and data mining or in AI systems without an express licence.

### Teams

- **McLaren** (terms last updated 10 February 2026) [MCL-ToU]:
  - 2.1: users may "print and download extracts … for your own personal, non-commercial use", provided that "(a) no documents or related graphics … are modified in any way; (b) no graphics … are used separately from the corresponding text; and (c) McLaren's copyright and trade mark notices and this permission notice appear in all copies."
  - 2.3: partner and sponsor marks shown on the site "belong to them … You may not copy or use them for any purpose, save as provided in clause 2.1."
  - 2.4: any other use "is prohibited".
  - 2.5: no part may be "included in any public or private electronic retrieval system".
  - Read strictly, cropping a car image out of a page and placing it on a dashboard tile fails 2.1(a) and 2.1(b), even for private use.
  - McLaren's media assets live in a Brandfolder portal (`https://brandfolder.com/portals/mclaren-racing-media`, linked from the mclaren.com/racing footer). The portal's own terms load only with JavaScript and were **not** read.
- **Mercedes-AMG F1:**
  - The website legal notice says team material "may only be used for personal use and may not be used or distributed in any way for commercial use … may not be re posted on other internet websites", and sponsor marks "may not be used or copied for any commercial purposes". [MGP-LN]
  - The media portal's general terms:
    - §1 Par. 7: use is "permitted exclusively to end-users".
    - §3 Par. 1: "non-exclusive utilization right for once-only use".
    - §3 Par. 2: "No commercial use of any kind whatsoever … without the prior, express, written consent of the respective copyright holder".
    - §3 Par. 4: any use "over and above one-time use" requires consent and a fee.
    - §3 Par. 7: no assignment to third parties.
    - §3 Par. 8: the copyright symbol must be visible.
    - Source: [MGP-GTC]
- **Ferrari:** "The Site and its Content can only be used for personal, educational or instructional purposes, with the exclusion of any direct or indirect commercial purpose … the Content may not be modified in any way." Trademarks "may not be reproduced in any form without the express authorization of Ferrari". [FER-L]
- **Red Bull Content Pool:** Red Bull Media House describes it as B2B, "free and rights-cleared for editorial and news purposes" [RBCP, from a search snippet; the page returned 404 when fetched]. The terms page on redbullcontentpool.com is a JavaScript single-page app and could not be read.
- **Aston Martin** uses a Canto portal (media.astonmartinf1.com redirects to astonmartinf1.canto.global), and **Williams** returns 401 on /media. Neither set of terms was read.
- **Not checked:** the other 2025 teams (Alpine, Haas, Racing Bulls, Sauber, Williams) and the 2026 entrants (Audi, Cadillac). Every terms page that could be read follows the same pattern: personal or editorial use, no commercial use, no redistribution.

### EA (F1 25)

- **EA content policy** (help article, "Updated 2 months ago") [EA-CP]:
  - "Our fans can use our game content, including gameplay and original characters, for personal uses or personal projects, including fan sites and videos."
  - "Don't use our game content for commercial purposes … Exception: Passive advertising, the monetization of videos via partner programs (like YouTube or Twitch), or the monetization of fan sites via passive banner ads is allowed."
  - Prohibited: "obtain assets through 'data mining', or distribute our games and content on file-sharing websites or torrents."
  - "Don't use our brands, studios, or games to make it seem like your project is endorsed by or affiliated with us including, for example, by using our logos or game art on your website. If you have a website or other location where you're displaying our game content, include the following statement: 'This [project/website] is not endorsed by or affiliated with EA or its licensors.'"
  - "Don't combine our game content with other third-party products, services, or brands."
  - "**Don't use licensed content.** Many of our games include content that we license from third parties … (for example, talent, brands, voices, music) … We do not provide you with any permission to use third-party content from our games. You use our game content at your own risk." F1 liveries are team and sponsor brands that EA licenses in, so in-game livery imagery falls under this clause.
  - Rights clearance: for "product demos" and similar uses, EA business partners use a form; "We will only respond if we are interested."
- **EA User Agreement** (last updated 14 May 2026) [EA-UA]:
  - §2: a "personal, limited, non-transferable … license … for your non-commercial use."
  - "You may not access, copy, modify or distribute any EA Service, Content or Entitlements … unless expressly authorized by EA or permitted by law. You may not reverse engineer or attempt to extract or otherwise use source code or other data from EA Services, unless expressly authorized by EA or permitted by law."
  - Rules of Conduct: "Do not upload or distribute any material that infringes another's intellectual property rights (including … protected logos or designs)."
- **Note on telemetry.** These terms are about art. pitwall's use of the documented UDP telemetry output is a separate matter: EA publishes the F1 25 UDP specification for exactly this purpose [EA-UDP]. Whether the UDP spec is "expressly authorized" under the User Agreement is outside this ticket.

### 3D model marketplaces

- **TurboSquid:**
  - Editorial Use Only models "depict branded or trademarked products owned by other companies".
  - Permitted uses are news, documentaries, and academic or research work.
  - Prohibited: commercial use, "using these models in video games", advertising, trade show or exhibit displays, and modifications that obscure the depicted IP.
  - "[TurboSquid] does not provide clearance for non-editorial uses." [TS-Ed]
  - The licence also says: "TurboSquid does not own or license any Depicted Intellectual Property"; "You may NOT re-distribute, publish, or make 3D Models available to any third party"; models in software distributed to the public "must be contained in proprietary formats so that they cannot be opened or imported". [TS-Lic, fetched as TurboSquid's published licence summary because the canonical `/3d-model-license` returned 403]
- **CGTrader:**
  - Royalty Free licence (§21A): use is allowed only as an "Incorporated Product" (one the product "cannot be extracted" from). Products "may not be sold, given, or assigned to another person or entity in the form it is downloaded"; "The resale or redistribution by the Buyer of any Product … is expressly prohibited." For software, "you must take all commercially reasonable measures to prevent the end user from gaining access to the Product."
  - Editorial (§22.2): "only … for legitimate, editorial purposes"; such products "may contain material that is not released from its rights holders."
  - §20.3: the buyer is responsible for any further licensing or clearance. [CGT-T&C]
  - CGTrader's help article on Royalty Free versus Editorial says Editorial allows personal non-commercial projects. It was blocked (403) and is cited from search results only, so the conflict with the T&C is unresolved.
- **Sketchfab:**
  - The Store has closed and models can no longer be bought or sold. Free downloads remain under their uploaders' licences, usually Creative Commons. [SKF-help]
  - Help articles quoted in search results say the Standard licence excluded identifiable third-party branding, and that Editorial-licence models could not be used commercially. Those Store licences no longer apply to new purchases.
- **Fab (Epic):** offers CC-BY and a Standard licence (Personal and Professional tiers by revenue) [FAB-doc]. The EULA's terms on third-party IP and redistribution could not be read (403). Open question.
- **Creative Commons generally:** "Patent and trademark rights are not licensed under this Public License" (CC BY 4.0 §2(b)(2)). Publicity and personality rights are not licensed either (§2(b)(1)). [CC-BY4] A free-licensed F1 model is therefore free only as to copyright; team and sponsor marks remain the owners'.

### Fan renders and free photos

- **Wikimedia Commons** holds free-licensed race photos of 2025 cars; the MCL39 Wikipedia article uses several [WP-MCL39]. The Commons "Trademarked" tag reads: "This work includes material that may be protected as a trademark in some jurisdictions. If you want to use it, you have to ensure that you have the legal right to do so." [WMC-TM]
- **Fan renders:** the artist owns the copyright in the render, and the marks on the livery remain with the team and sponsors. This follows from the general split between copyright and trademark seen in every source above; the terms of individual art platforms were not researched.

### Comparable community tools (prior art)

- **SimHub dashboards** for the F1 games tend to give users an image slot rather than bundling art; for example, one lets you add a "My Team" image through SimHub's image manager [SH-1]. Others ship team logos and colours inside paid or free downloads [SH-2]. Neither approach is evidence of permission.
- **P1 Telemetry** (iOS) carries F1's recommended disclaimer: "unofficial … not associated in any way with the Formula 1 companies" [P1].

## How the Splunk dashboard likely got its McLaren render

**Established facts:**

- Splunk has been McLaren's "Official Technology Partner" since 2020. The 2021 extension put Splunk branding on the MCL35M sidepods, cockpits and race suits. [SPL-MCL; MCL-SPL21]
- Cisco acquired Splunk in 2024. Reports of a multi-year Cisco–McLaren extension in July 2026 say it folds the Splunk relationship in under Cisco. [CISCO-26, trade-press report]
- Splunk builds F1-game telemetry demos with McLaren framing. At Cisco Live Amsterdam in 2025, three rigs ran the F1 2024 game, with UDP telemetry converted to JSON and dashboards showing speed, RPM, gear, tyre temperature and brake pressure. [SPL-CL25] Splunk's Dashboard Studio has been in the product since version 8.2. [SPL-DS]
- McLaren distributes media and brand assets through a hosted portal (Brandfolder) [MCL-ToU footer link]. For the MCL34 launch, it pre-staged assets for "media and racing partners" on StoryStream [SS-MCL34, vendor case study].

**Inference (not confirmed):** the MCL39 render most likely came from McLaren itself, through partner-asset channels such as the Brandfolder portal or a direct supply from McLaren's partnerships team, under the marketing rights in the Splunk/Cisco sponsorship contract. No source found shows the specific dashboard or credits the render, and the sponsorship contract is not public. **pitwall has no such contract, so the Splunk route is not available to it.**

## Architectural options that keep pitwall free of redistributed trademarked art

Options are ordered from least to most rights exposure for the pitwall project itself.

1. **Stylised fallback in team colours** (shipped by default)
   - An original top-down or side car outline that pitwall draws itself, as SVG.
   - Filled with a per-team palette chosen from the team ID in the game's telemetry.
   - The tyre and brake overlays anchor to wheel positions that pitwall defines.
   - No logos, sponsor marks or F1 typeface. F1 lists "Official Typeface" as Other IP that "cannot be used in apps" [F1-G].
   - Risk to check: colour combinations as trade dress or colour marks. This was **not** researched; use colours only, never sponsor graphics.
2. **User-supplied livery packs loaded from a local folder** (opt-in)
   - The collector reads a folder of images plus a manifest from a user-chosen path and serves them on the LAN.
   - The manifest maps telemetry team ID (and season or game) to an image file, and records the wheel-centre coordinates and scale so tyre and brake overlays line up on any art.
   - The repository and binaries ship **no** real-livery files, no download script and no links to sources.
   - The docs describe the file format only.
   - The user is responsible for the rights to whatever they load. For private use that could be their own screenshots under the EA policy's personal-use allowance, with the third-party-content caveat [EA-CP].
   - This is the pattern SimHub users already rely on [SH-1].
3. **Per-layout choice of livery mode**
   - A layout setting picks "stylised" or "pack", so a streamer or presenter can switch to the stylised fallback for public sessions without editing files.
   - A preset should default to stylised.
4. **Runtime fetch from F1 or team servers** (hot-linking): **not recommended**. Nothing is redistributed, but the images are still used inside an app, against F1's guidelines [F1-G] and McLaren 2.5 [MCL-ToU].
5. **Licensed real art**: the only route that covers (b) to (d) without caveats. Write to brandprotection@f1.com for F1 rights and to each team for team rights [F1-G]. This is realistic only for a funded or commercial version.

## F1 trademark rules relevant to naming and UI

From [F1-G] unless noted otherwise:

- **No F1 logos anywhere**: not in the UI, not in the favicon or app icon. Logos may not be incorporated into "website icons, or brand identities".
- **Product name and domain.** Marks cannot be used in "company, trade, product, or service names, social handles, or trade marks". "pitwall" is fine; "F1 Pitwall" or "pitwall F1" is not. Domains: a path such as `example.com/formula1` is tolerated, a subdomain such as `formula1.example.com` is not, and marks may not be used in commercial domain names.
- **Use word marks to inform, not to brand.** Write them as adjectives ("FORMULA 1 race"), never as nouns or in the possessive. Do not stylise them or set them in a different font from the surrounding text. ™ is "generally expected" in headlines.
- **Disclaimer.** F1 asks unofficial sites for this footer text: "This website is unofficial and is not associated in any way with the Formula 1 companies. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX and related marks are trade marks of Formula One Licensing B.V."
- **EA disclaimer** wherever game content is shown: "This [project/website] is not endorsed by or affiliated with EA or its licensors." Do not use EA logos or game art to imply endorsement [EA-CP].
- **No F1 official typeface** (the F1 display fonts) in the UI.
- **Team names in the UI.** The F1 guidelines defer team rights to the teams and give no explicit allowance. Showing team names as factual labels taken from telemetry is common practice, but no source checked addresses it. Open question.

## Open questions

1. **McLaren Brandfolder portal and Red Bull Content Pool terms.** Both are JavaScript-rendered and were not read. They need a browser session, and accreditation may be required.
2. **Fab EULA** clauses on third-party IP and redistribution (403).
3. **CGTrader conflict.** The help article allows "personal projects" under the Editorial licence; the T&C §22.2 says "editorial purposes" only.
4. **Does F1's "Simulators" clause reach companion apps** that read simulator telemetry? The wording ("in association with motorsport simulators") is ambiguous. Ask brandprotection@f1.com if pitwall becomes a public or commercial project.
5. **Team colours.** Can a team's colour scheme, without marks, be protected as trade dress in the relevant jurisdictions? Not researched.
6. **Team and driver names** as factual labels in the UI (nominative use). No source checked addresses this.
7. **Monetised streams.** EA allows partner-programme monetisation of game content [EA-CP], while F1 excludes "materially commercial" fan use [F1-G]. How the two interact for a monetised stream showing a pitwall tile with real livery art is not settled by either text.
8. **Season coverage.** The F1 25 2026 Season Pack (released 3 June 2026) adds Cadillac and replaces Sauber with Audi, with 2026-regulation cars [PF1-26]. A forum reply says the 2026 teams use team IDs 220–230 in the UDP spec [EA-UDP26, search snippet; the thread returned 403]. Livery packs and palettes therefore need to be keyed by season as well as team.

## Sources

- [F1-LN] Formula 1, Legal Notices: https://www.formula1.com/en/toolbar/legal-notices.html (also https://www.formula1.com/en/information/legal-notices.7egvZU48hzrypubGBNcQKt)
- [F1-G] Formula 1, Guidelines (trade mark and IP use): https://www.formula1.com/en/information/guidelines.4EOKE9RRqevL4niTK9kWyt
- [MCL-ToU] McLaren Racing, Terms of Use (last updated 10 Feb 2026): https://www.mclaren.com/racing/terms-of-use/ ; media portal: https://brandfolder.com/portals/mclaren-racing-media
- [MGP-LN] Mercedes-AMG F1, Legal notice: https://www.mercedesamgf1.com/legal-notice
- [MGP-GTC] Mercedes-AMG F1 media site, General Terms and Conditions: https://media.mercedesamgf1.com/marsF1/pub/terms_en.xhtml
- [FER-L] Ferrari, Legal / Terms and Conditions of Use: https://www.ferrari.com/en-EN/legal
- [RBCP] Red Bull Media House, Red Bull Content Pool (search snippet; 404 on fetch): https://redbullmediahouse.com/en/red-bull-content-pool ; portal: https://www.redbullcontentpool.com/
- [EA-CP] EA, "EA's content policy": https://help.ea.com/en/articles/security-and-rules/ea-content-policy/
- [EA-UA] EA, User Agreement (last updated 14 May 2026): https://www.ea.com/legal/user-agreement
- [EA-UDP] EA, F1 25 UDP specification: https://forums.ea.com/blog/f1-games-game-info-hub-en/f1%c2%ae-25-udp-specification/12187347
- [EA-UDP26] EA forums, F1 25 2026 Season Pack UDP specification thread (search snippet; 403): https://forums.ea.com/discussions/f1-25-general-discussion-en/discussion-f1%C2%AE25-2026-season-pack-udp-specification/13424444
- [PF1-26] PlanetF1, "F1 25's 2026 season update adds Audi, Cadillac and Madring": https://www.planetf1.com/news/f1-26-ea-sports-download-season-pack-available ; Fanatec launch note: https://www.fanatec.com/eu/en/explorer/products/racing-wheels-wheel-bases/f1-25-2026-season-pack-launches-today/
- [TS-Ed] TurboSquid, Editorial Use Information: https://www.turbosquid.com/help/en/articles/9937424-editorial-use-information
- [TS-Lic] TurboSquid 3D Model License (published summary): https://blog.turbosquid.com/turbosquid-3d-model-license/ (canonical https://www.turbosquid.com/3d-model-license returned 403)
- [CGT-T&C] CGTrader, General Terms and Conditions: https://www.cgtrader.com/pages/terms-and-conditions ; help article (search only, 403): https://help.cgtrader.com/hc/en-us/articles/360015193418-What-is-the-difference-between-Royalty-Free-and-Editorial-licenses
- [SKF-help] Sketchfab Help, Store licence FAQ (Store closed): https://help.sketchfab.com/en/articles/16152225-store-license-usage-faq ; https://help.sketchfab.com/en/articles/16152221-choosing-your-model-s-license
- [FAB-doc] Epic, Licenses and pricing in Fab: https://dev.epicgames.com/documentation/en-us/fab/licenses-and-pricing-in-fab ; EULA (403): https://www.fab.com/eula
- [CC-BY4] Creative Commons BY 4.0 legal code: https://creativecommons.org/licenses/by/4.0/legalcode.en
- [WMC-TM] Wikimedia Commons, Template:Trademarked: https://commons.wikimedia.org/wiki/Template:Trademarked
- [WP-MCL39] Wikipedia, McLaren MCL39: https://en.wikipedia.org/wiki/McLaren_MCL39
- [SPL-MCL] Splunk, Official McLaren Technology Partner: https://www.splunk.com/en_us/mclaren.html
- [MCL-SPL21] McLaren, Splunk partnership extension (8 Feb 2021): https://www.mclaren.com/racing/partners/splunk/mclaren-racing-and-splunk-announce-multi-year-formula-1-partnership-extension/
- [SPL-CL25] Splunk blog, "Post-Race Analysis: How Splunk and McLaren Drove Data at Cisco Live" (24 Mar 2025): https://www.splunk.com/en_us/blog/observability/how-splunk-and-mclaren-drove-data-at-cisco-live.html
- [SPL-DS] Splunk blog (DE), F1 game telemetry in Splunk / Dashboard Studio (30 Aug 2022): https://www.splunk.com/de_de/blog/tips-and-tricks/monitoring-der-telemetrie-eines-virtuellen-formel-1-autos.html
- [CISCO-26] Sportcal, "McLaren extends Cisco partnership in AI-focused deal" (July 2026): https://www.sportcal.com/newsletters/mclaren-extends-cisco-partnership-in-ai-focused-deal
- [SS-MCL34] StoryStream case study, MCL34 launch: https://storystream.ai/case-studies/mclaren-f1-launches-mcl34-through-storystream
- [SH-1] Overtake, Ultimate F1 22 Dashboard (SimHub): https://www.overtake.gg/threads/ultimate-f1-22-dashboard-simhub-patreon.241585/
- [SH-2] RaceDepartment, F1 2022 TV overlays for SimHub: https://www.racedepartment.com/downloads/official-f1-2022-tv-overlays-for-assetto-corsa-simhub.50569/
- [P1] P1 Telemetry, App Store listing: https://apps.apple.com/app/id6452589420
