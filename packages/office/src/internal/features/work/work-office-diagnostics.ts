import {
  serializePreservableDocxCellPropertyRevision,
  serializePreservableDocxRowPropertyRevision,
  serializePreservableDocxSectionPropertyRevision,
  serializePreservableDocxTablePropertyRevision,
} from './work-document-table-property-revision';
import { diagnoseDocxBookmarksAndLinks } from './work-docx-bookmark-diagnostics';
import { diagnoseDocxCaptions } from './work-docx-caption-diagnostics';
import { isSupportedDocxCellFormattingChange } from './work-docx-cell-format-change-import';
import {
  inspectDocxMoveRangeCompanions,
  isSupportedDocxMoveChange,
  supportedDocxMovePairCount,
} from './work-docx-change-import';
import { diagnoseDocxCitations } from './work-docx-citation-diagnostics';
import { inspectDocxConnectors } from './work-docx-connector-diagnostics';
import { inspectDocxContentControls } from './work-docx-content-control-import';
import { diagnoseDocxEmphasisMarks } from './work-docx-emphasis-diagnostics';
import { diagnoseDocxEquations } from './work-docx-equation-diagnostics';
import { diagnoseDocxHiddenText } from './work-docx-hidden-text-diagnostics';
import { readDocxImageTransform } from './work-docx-image-transform';
import { diagnoseDocxKerning } from './work-docx-kerning-diagnostics';
import { diagnoseDocxLegacyTextEffects } from './work-docx-legacy-text-effects-diagnostics';
import { diagnoseDocxNotes } from './work-docx-note-diagnostics';
import { isSupportedDocxNumberingChange } from './work-docx-numbering-change-import';
import { diagnoseDocxOpenTypeTypography } from './work-docx-opentype-diagnostics';
import { diagnoseDocxPageBorders } from './work-docx-page-borders-diagnostics';
import { diagnoseDocxPageChrome } from './work-docx-page-chrome-diagnostics';
import { diagnoseDocxPageMargins } from './work-docx-page-margins-diagnostics';
import { diagnoseDocxPageSize } from './work-docx-page-size-diagnostics';
import { diagnoseDocxParagraphBorders } from './work-docx-paragraph-borders-diagnostics';
import { parseDocxParagraphDefaultCollapsed } from './work-docx-paragraph-default-collapsed';
import { isSupportedDocxParagraphFormattingChange } from './work-docx-paragraph-format-change-import';
import {
  createDocxExternalHyperlinkTargets,
  createDocxImageEmbedTargets,
  isIsolatedDocxParagraphBreakMarkChange,
  isSupportedDocxParagraphMarkChange,
} from './work-docx-paragraph-mark-change-import';
import { diagnoseDocxParagraphShading } from './work-docx-paragraph-shading-diagnostics';
import { diagnoseDocxProofing } from './work-docx-proofing-diagnostics';
import { isSupportedDocxRowFormattingChange } from './work-docx-row-format-change-import';
import { diagnoseDocxRunBorders } from './work-docx-run-border-diagnostics';
import { diagnoseDocxRunFonts } from './work-docx-run-fonts-diagnostics';
import { isSupportedDocxRunFormattingChange } from './work-docx-run-formatting-import';
import { diagnoseDocxRunShading } from './work-docx-run-shading-diagnostics';
import { isSupportedDocxSectionFormattingChange } from './work-docx-section-format-change-import';
import { isSupportedDocxTableFormattingChange } from './work-docx-table-format-change-import';
import { inspectDocxTextBoxes } from './work-docx-text-box-import';
import {
  attribute,
  descendants,
  directChildren,
  OoxmlPackage,
} from './work-ooxml-package';
import type {
  WorkCompatibilityIssue,
  WorkCompatibilityReport,
} from './work-types';

interface ConversionMessage {
  type: string;
  message: string;
}


export async function analyzeDocxCompatibility(
  file: File,
  messages: ConversionMessage[],
  sourcePackage?: OoxmlPackage | null,
): Promise<WorkCompatibilityReport> {
  const issues: WorkCompatibilityIssue[] = [
    issue(
      'docx.page-layout',
      'Page layout',
      'Per-section paper size, orientation, exact native page margins, one-to-six equal or custom-width columns, section breaks, and explicit page breaks are preserved; exact pagination and line wrapping may normalize.',
    ),
    issue(
      'docx.package-state',
      'OOXML package state',
      'Source-backed export retains safe source-only parts byte-for-byte and reconnects their content types and relationships. Passive relationship-free extensions in settings XML, uniquely matched style or numbering identities, stable picture drawings, unchanged native-identity paragraphs, stable table, row, or cell scopes, and uniquely matched footnote, endnote, comment, or comment-thread records are preserved against the final package graph. Text-stable direct, supported-hyperlink-wrapped, and eligible static-content-control note and comment runs retain paragraph, table, wrapper, run, and run-property metadata; native note tables and DrawingML pictures remain editable, note image relationships and media targets are repaired and validated, bounded structured OMML equations remain atomically updateable in supported Word stories, uniquely matched table-bearing rich-text controls are reconstructed, safe web, mail, and internal note/comment links are rebound to collision-free final relationship IDs, and content-control ID collisions are rewritten. Native note, comment, and note-picture identities survive reorderings, comment durable IDs are rebound to regenerated paragraph IDs, and eligible source font-table metadata is retained. Generated Word semantics remain authoritative; source-only, changed-text or table structure, malformed, duplicate or cross-kind, active, unsupported wrappers, mixed-semantic, unsafe relationship-bound, legacy VML, non-picture drawings, or unsupported rich content may normalize.',
      'info',
    ),
  ];
  for (const message of messages) {
    issues.push(
      issue(
        `docx.converter.${message.type}`,
        'Document conversion',
        message.message,
        message.type === 'error' ? 'error' : 'warning',
      ),
    );
  }

  try {
    const archive =
      sourcePackage === undefined
        ? await OoxmlPackage.load(await file.arrayBuffer())
        : sourcePackage;
    if (!archive) throw new Error('The DOCX package could not be loaded.');
    const packagePaths = archive.paths('');
    const normalizedPackagePaths = packagePaths.map((path) =>
      path.toLowerCase(),
    );
    const document = archive.has('word/document.xml')
      ? await archive.xml('word/document.xml')
      : null;
    const documentRelationships = [
      ...(await archive.relationships('word/document.xml')).values(),
    ];
    const externalHyperlinks = createDocxExternalHyperlinkTargets(
      documentRelationships,
    );
    const imageEmbeds = createDocxImageEmbedTargets(documentRelationships);
    if (
      normalizedPackagePaths.some((path) => path.startsWith('_xmlsignatures/'))
    ) {
      issues.push(
        issue(
          'docx.signatures.removed',
          'Digital signatures',
          'Package signatures become invalid after an edit and are deliberately omitted from the exported DOCX.',
        ),
      );
    }
    if (
      normalizedPackagePaths.some(
        (path) =>
          path === 'word/vbaproject.bin' ||
          path === 'word/vbadata.xml' ||
          path.startsWith('word/activex/') ||
          path.startsWith('customui/'),
      )
    ) {
      issues.push(
        issue(
          'docx.active-content.removed',
          'Active content',
          'VBA, ActiveX, and custom-ribbon parts are never executed and are omitted from the macro-free DOCX export.',
        ),
      );
    }
    if (archive.has('word/fontTable.xml')) {
      const fontTable = await archive.xml('word/fontTable.xml');
      const embeddedFontReferences = [
        'embedRegular',
        'embedBold',
        'embedItalic',
        'embedBoldItalic',
      ].reduce(
        (count, localName) => count + descendants(fontTable, localName).length,
        0,
      );
      if (embeddedFontReferences) {
        issues.push(
          issue(
            'docx.embedded-fonts',
            'Embedded fonts',
            `${embeddedFontReferences} embedded font reference(s) were found. Eligible internal obfuscated-font payloads, metadata, relationships, and remapped IDs survive source-backed DOCX export, but the browser editor, preview, and PDF renderer use registered A3S fonts or substitution and may wrap text differently.`,
          ),
        );
      }
    }
    if (archive.has('word/comments.xml')) {
      const comments = await archive.xml('word/comments.xml');
      const commentDefinitions = descendants(comments, 'comment');
      if (commentDefinitions.length) {
        issues.push(
          issue(
            'docx.comments',
            'Comments',
            `${commentDefinitions.length} comment or reply record(s), stable native IDs, text anchors, authors, dates, thread relationships, resolved state, eligible durable IDs, unchanged direct-run formatting, safely rebound web, mail, or internal hyperlinks, and eligible static text content controls are preserved. Comment text remains editable.`,
            'info',
          ),
        );
        if (
          ['tbl', 'drawing', 'pict', 'sdt', 'altChunk'].some(
            (name) => descendants(comments, name).length,
          ) ||
          descendants(comments, 'rPr').length ||
          descendants(comments, 'hyperlink').length
        ) {
          issues.push(
            issue(
              'docx.comments.formatting',
              'Comment formatting',
              'Relationship-free formatting on uniquely matched unchanged direct text runs and eligible reconstructed control runs is retained. Supported HTTP(S), mailto, and internal hyperlink wrappers keep stable formatting, safe tooltips, and collision-free relationship IDs. Eligible static rich-text or plain-text controls retain inline or contiguous block wrappers, aliases, tags, locks, rewritten collision IDs, appearance, color, end formatting, and passive metadata. Editing comment text or using active bindings, placeholder state, form or nested controls, tables, images, math, mixed hyperlinks, unsupported wrappers, malformed relationships, or relationship-bound properties normalizes the affected content to safe plain text.',
            ),
          );
        }
        if (
          archive.has('word/commentsExtensible.xml') ||
          archive.has('word/people.xml')
        ) {
          issues.push(
            issue(
              'docx.comments.modern-sidecars',
              'Modern comment metadata',
              'Durable comment IDs are safely rebound, but follow-up flags, reactions, and people metadata in modern comment sidecars are not editable and are omitted rather than reattached to a stale comment identity.',
            ),
          );
        }
      }
    }
    issues.push(...(await diagnoseDocxNotes(archive, document)));
    issues.push(...(await diagnoseDocxEquations(archive, document)));
    if (document) {
      issues.push(...(await diagnoseDocxKerning(archive, document)));
      issues.push(...(await diagnoseDocxOpenTypeTypography(archive, document)));
      issues.push(...(await diagnoseDocxEmphasisMarks(archive, document)));
      issues.push(...(await diagnoseDocxHiddenText(archive, document)));
      issues.push(...(await diagnoseDocxLegacyTextEffects(archive, document)));
      issues.push(...(await diagnoseDocxRunBorders(archive, document)));
      issues.push(...(await diagnoseDocxRunShading(archive, document)));
      issues.push(...(await diagnoseDocxProofing(archive, document)));
      issues.push(...(await diagnoseDocxRunFonts(archive, document)));
    }
    issues.push(...(await diagnoseDocxPageChrome(archive)));
    if (archive.paths('word/embeddings/').length) {
      issues.push(
        issue(
          'docx.embedded',
          'Embedded objects',
          'Embedded Office and OLE payload parts plus safe relationships are retained in source-backed exports, but Work does not render or edit their object anchors; placement inside regenerated document XML may normalize.',
        ),
      );
    }
    if (document) {
      issues.push(...diagnoseDocxPageSize(document));
      issues.push(...(await diagnoseDocxPageMargins(archive, document)));
      issues.push(...(await diagnoseDocxPageBorders(archive, document)));
      issues.push(...(await diagnoseDocxParagraphBorders(archive, document)));
      issues.push(...(await diagnoseDocxParagraphShading(archive, document)));
      const defaultCollapsedStates = descendants(document, 'pPr')
        .map(parseDocxParagraphDefaultCollapsed)
        .filter((state) => state.status !== 'absent');
      if (defaultCollapsedStates.length) {
        const invalid = defaultCollapsedStates.filter(
          (state) => state.status === 'invalid',
        ).length;
        issues.push(
          issue(
            'docx.headings.default-collapsed',
            'Default-collapsed headings',
            invalid
              ? `Office 2013 default-collapsed heading state is preserved for exact core Word on/off values, but ${invalid} malformed or duplicated paragraph property value(s) are ignored instead of inheriting a stale state.`
              : "Office 2013 default-collapsed heading state is preserved as native paragraph metadata. Browser content remains expanded and editable because the property controls Word's initial document view rather than content visibility.",
            invalid ? 'warning' : 'info',
          ),
        );
      }
      if (descendants(document, 'AlternateContent').length) {
        issues.push(
          issue(
            'docx.markup-compatibility',
            'Markup compatibility',
            '`mc:AlternateContent` branches outside supported stable paragraph, picture-drawing, table, row, or cell scopes can normalize. Safe settings-level and uniquely matched passive branches are preserved separately, but source-backed package preservation does not imply general body-content fallback preservation.',
          ),
        );
      }
      issues.push(...(await diagnoseDocxBookmarksAndLinks(archive, document)));
      issues.push(...(await diagnoseDocxCitations(archive, document)));
      const captionDiagnostics = diagnoseDocxCaptions(document);
      issues.push(...captionDiagnostics.issues);
      const textBoxInspection = inspectDocxTextBoxes(document);
      const connectorInspection = inspectDocxConnectors(document);
      if (textBoxInspection.supported) {
        issues.push(
          issue(
            'docx.text-boxes',
            'Text boxes and shapes',
            `${textBoxInspection.supported} isolated WPS text box or common shape(s) remain editable with bounded DrawingML geometry (rectangle, rounded rectangle, ellipse, diamond, triangle, parallelogram, or hexagon), inline or floating layout, safe page-relative offsets, fill, outline, padding, and vertical alignment. Mixed paragraphs and connectors remain on the normal DOCX compatibility path.`,
            'info',
          ),
        );
      }
      if (textBoxInspection.unsupported) {
        issues.push(
          issue(
            'docx.text-boxes.unsupported',
            'Text boxes and shapes',
            `${textBoxInspection.unsupported} WPS text box or shape(s) mix a drawing with other paragraph content, use unsupported geometry, or have malformed shape content. Only isolated text-bearing paragraphs in the bounded preset set are converted to editable shapes; connectors and the rest may normalize during browser conversion.`,
          ),
        );
      }
      if (connectorInspection.detected) {
        issues.push(
          issue(
            'docx.connectors',
            'Connectors',
            `${connectorInspection.detected} WPS VML straight connector(s) were detected. Isolated connectors are converted to the bounded editable Writer model with explicit endpoints, line color/width, and optional arrow styles; elbow or curved routing, relationship-bound anchors, and unsupported mixed paragraphs remain on the compatibility path and may normalize when edited or regenerated.`,
          ),
        );
      }
      const contentControlInspection = inspectDocxContentControls(document);
      if (contentControlInspection.supported) {
        issues.push(
          issue(
            'docx.content-controls',
            'Content controls',
            `${contentControlInspection.supported} inline text, rich-text, checkbox, drop-down list, combo box, date, or passively data-bound content control(s) remain editable with aliases, tags, bounded locking, multiline text, appearance, color, checkbox state, list options, date values, custom-XML binding metadata, and native DOCX w:sdt round-tripping. Placeholders, live custom-XML sync, picture controls, and broader body-level block or relationship-bound controls remain outside the safe subset.`,
            'info',
          ),
        );
      }
      if (contentControlInspection.unsupported) {
        issues.push(
          issue(
            'docx.content-controls.unsupported',
            'Content controls',
            `${contentControlInspection.unsupported} body content control(s) use active bindings, placeholders, form semantics, unsupported structure, or malformed properties. They are flattened to safe editable text during browser conversion instead of being revived with incomplete behavior.`,
          ),
        );
      }
      if (descendants(document, 'tbl').length) {
        issues.push(
          issue(
            'docx.tables',
            'Tables',
            'Table rows, non-splitting rows, repeated headers, cell shading, vertical alignment, per-edge borders, and paragraph-boundary row continuation remain editable. Native row identities survive body and page-chrome HTML; uniquely matched passive extensions on stable table, row, and cell scopes and their property nodes are retained, while duplicate, cross-kind, relationship-bound, or semantic branches fail closed. Layout algorithm, preferred auto, percentage, or pixel width, alignment, indent, percentage or pixel column widths, table-level cell margins, and cell-level margin overrides are preserved independently across edit, preview, and DOCX export. Percentage column preferences use pixel grid widths as responsive browser fallbacks and remain coherent through merged cells. Nested tables retain independent edit and DOCX geometry, and tall outer rows can paginate at nested-row boundaries. Row-spanning cells continue through every covered physical row with in-cell page breaks and contiguous content ranges; combined row and column spans retain native DOCX merge semantics. Common inherited Word table styles apply whole-table, banded row or column, first or last row or column, corner-cell, border, fill, text-emphasis, paragraph alignment, direction, indents, spacing, line, pagination, and tab-stop rules before direct table, cell, paragraph, and run formatting. Theme tint and shade values resolve to stable RGB for edit and preview, while untouched run colors, run shading, cell fills, and per-edge borders retain their semantic theme references on export; explicit color edits use direct RGB. Less-common conditional paragraph properties may be normalized.',
          ),
        );
      }
      const drawingCount = descendants(document, 'drawing').length;
      const textBoxDrawingCount =
        textBoxInspection.supported + textBoxInspection.unsupported;
      const pictCount = descendants(document, 'pict').length;
      if (
        drawingCount > textBoxDrawingCount ||
        pictCount > connectorInspection.connectorPictContainers
      ) {
        const unsupportedImageTransforms =
          docxUnsupportedImageTransformCount(document);
        issues.push(
          issue(
            'docx.images',
            'Images',
            'Inline images and supported square, tight, through, top-and-bottom, or no-wrap free-floating images remain embedded with editable size, alternative text, alignment, wrap side and distance where applicable, signed horizontal or vertical offsets relative to the column, paragraph, margin, or page, four-edge percentage cropping, 90-degree rotation, horizontal or vertical reflection, and drawing-layer options. Aligned and precise DOCX anchors retain their distinct position semantics; inline and floating crop geometry round-trips as DrawingML source rectangles; tight or through anchors retain their ordered wrap polygons; no-wrap anchors remain outside the body text flow and use behind-text placement to choose their side of the text; and supported floating images preserve relative Z-order, overlap and cell-layout policy, and anchor locking. Inline and floating pictures retain unique drawing-property and anchor IDs together with their edit IDs. Copies receive independent identities while move, delete, undo, and relationship regeneration retain the original object identity. Arbitrary-angle or malformed picture transforms and unsupported drawing types may be normalized.',
          ),
        );
        if (unsupportedImageTransforms) {
          issues.push(
            issue(
              'docx.images.transform',
              'Image transforms',
              `${unsupportedImageTransforms} picture transform(s) use an arbitrary angle or malformed rotation/reflection value. Work imports the editable 90-degree/reflection subset and normalizes the unsupported transform instead of exposing an inexact control.`,
            ),
          );
        }
      }
      const textRevisions = [
        ...descendants(document, 'ins'),
        ...descendants(document, 'del'),
      ];
      const moveFromRevisions = descendants(document, 'moveFrom');
      const moveToRevisions = descendants(document, 'moveTo');
      const moveRevisionCount =
        moveFromRevisions.length + moveToRevisions.length;
      const supportedMovePairCount = supportedDocxMovePairCount(document);
      const supportedMoveRevisionCount = [
        ...moveFromRevisions,
        ...moveToRevisions,
      ].filter(isSupportedDocxMoveChange).length;
      const moveRangeCompanions = inspectDocxMoveRangeCompanions(document);
      const runFormattingRevisions = descendants(document, 'rPrChange');
      const paragraphFormattingRevisions = descendants(document, 'pPrChange');
      const paragraphMarkRevisions = [
        ...descendants(document, 'ins'),
        ...descendants(document, 'del'),
      ].filter(
        (revision) =>
          revision.parentElement?.localName === 'rPr' &&
          revision.parentElement.parentElement?.localName === 'pPr' &&
          revision.parentElement.parentElement.parentElement?.localName === 'p',
      );
      const numberingRevisions = descendants(document, 'numberingChange');
      const supportedRunFormattingRevisionCount = runFormattingRevisions.filter(
        isSupportedDocxRunFormattingChange,
      ).length;
      const supportedParagraphFormattingRevisionCount =
        paragraphFormattingRevisions.filter(
          isSupportedDocxParagraphFormattingChange,
        ).length;
      const supportedParagraphMarkRevisions = paragraphMarkRevisions.filter(
        (revision) =>
          isSupportedDocxParagraphMarkChange(
            revision,
            externalHyperlinks,
            imageEmbeds,
          ),
      );
      const supportedParagraphMarkRevisionCount =
        supportedParagraphMarkRevisions.length;
      const isolatedParagraphBreakMarkRevisions = paragraphMarkRevisions.filter(
        (revision) =>
          !isSupportedDocxParagraphMarkChange(
            revision,
            externalHyperlinks,
            imageEmbeds,
          ) &&
          isIsolatedDocxParagraphBreakMarkChange(
            revision,
            externalHyperlinks,
            imageEmbeds,
          ),
      );
      const isolatedParagraphBreakMarkRevisionCount =
        isolatedParagraphBreakMarkRevisions.length;
      const supportedNumberingRevisionCount = numberingRevisions.filter(
        isSupportedDocxNumberingChange,
      ).length;
      const tablePropertyRevisions = descendants(document, 'tblPrChange');
      const supportedTableFormattingRevisionCount =
        tablePropertyRevisions.filter(
          isSupportedDocxTableFormattingChange,
        ).length;
      const preservableTablePropertyRevisionCount =
        tablePropertyRevisions.filter((change) => {
          if (isSupportedDocxTableFormattingChange(change)) return false;
          const properties = change.parentElement;
          if (properties?.localName !== 'tblPr') return false;
          return (
            serializePreservableDocxTablePropertyRevision(properties) !== null
          );
        }).length;
      const unpreservableTablePropertyRevisionCount =
        tablePropertyRevisions.length -
        preservableTablePropertyRevisionCount -
        supportedTableFormattingRevisionCount;
      const rowPropertyRevisions = descendants(document, 'trPrChange');
      const supportedRowFormattingRevisionCount = rowPropertyRevisions.filter(
        isSupportedDocxRowFormattingChange,
      ).length;
      const preservableRowPropertyRevisionCount = rowPropertyRevisions.filter(
        (change) => {
          if (isSupportedDocxRowFormattingChange(change)) return false;
          const properties = change.parentElement;
          if (properties?.localName !== 'trPr') return false;
          return (
            serializePreservableDocxRowPropertyRevision(properties) !== null
          );
        },
      ).length;
      const unpreservableRowPropertyRevisionCount =
        rowPropertyRevisions.length -
        preservableRowPropertyRevisionCount -
        supportedRowFormattingRevisionCount;
      const cellPropertyRevisions = descendants(document, 'tcPrChange');
      const supportedCellFormattingRevisionCount = cellPropertyRevisions.filter(
        isSupportedDocxCellFormattingChange,
      ).length;
      const preservableCellPropertyRevisionCount = cellPropertyRevisions.filter(
        (change) => {
          if (isSupportedDocxCellFormattingChange(change)) return false;
          const properties = change.parentElement;
          if (properties?.localName !== 'tcPr') return false;
          return (
            serializePreservableDocxCellPropertyRevision(properties) !== null
          );
        },
      ).length;
      const unpreservableCellPropertyRevisionCount =
        cellPropertyRevisions.length -
        preservableCellPropertyRevisionCount -
        supportedCellFormattingRevisionCount;
      if (
        textRevisions.some(
          (revision) =>
            descendants(revision, 't').length ||
            descendants(revision, 'delText').length,
        )
      ) {
        issues.push(
          issue(
            'docx.revisions',
            'Tracked changes',
            'Body-text insertions and deletions preserve their author and date, remain reviewable in Work, and round-trip as native DOCX revisions.',
            'info',
          ),
        );
      }
      if (supportedRunFormattingRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.formatting',
            'Character-formatting revisions',
            `${supportedRunFormattingRevisionCount} bounded character-formatting revision(s) preserve author, date, current formatting, and prior bold, italic, underline, strike, text case, hidden text, outline, shadow, emboss, imprint, subscript, superscript, font, size, color, highlight, character borders, character shading, character scale, spacing, kerning threshold, emphasis mark, baseline position, grid state, proofing languages, and explicit proofing state. They remain reviewable in Work and round-trip as native w:rPrChange records.`,
            'info',
          ),
        );
      }
      if (supportedParagraphFormattingRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.paragraph-formatting',
            'Paragraph-formatting revisions',
            `${supportedParagraphFormattingRevisionCount} bounded paragraph-formatting revision(s) preserve author, date, current formatting, and prior alignment, direction, indentation, spacing, pagination, outline, tab-stop, border, shading, and collapsed state. They remain reviewable in Work and round-trip as native w:pPrChange records.`,
            'info',
          ),
        );
      }
      if (supportedTableFormattingRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.table-formatting',
            'Table-formatting revisions',
            `${supportedTableFormattingRevisionCount} relationship-free w:tblPrChange record(s) with a prior alignment, preferred-width, indent, default cell-margin, layout, bidiVisual, solid fill, tblLook, tblOverlap, relationship-free tblStyle, dxa tblCellSpacing, relationship-free tblStyleColBandSize, relationship-free tblStyleRowBandSize, direct-color tblBorders, relationship-free tblCaption, relationship-free tblDescription, and/or relationship-free attribute-only tblpPr snapshot remain reviewable in Work as table-formatting changes and round-trip as native w:tblPrChange records.`,
            'info',
          ),
        );
      }
      if (supportedRowFormattingRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.row-formatting',
            'Row-formatting revisions',
            `${supportedRowFormattingRevisionCount} relationship-free w:trPrChange record(s) with a prior cantSplit, tblHeader, trHeight, hidden, jc, gridBefore, gridAfter, wBefore, wAfter, cnfStyle, divId, and/or tblCellSpacing snapshot remain reviewable in Work as row-formatting changes and round-trip as native w:trPrChange records.`,
            'info',
          ),
        );
      }
      if (supportedCellFormattingRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.cell-formatting',
            'Cell-formatting revisions',
            `${supportedCellFormattingRevisionCount} relationship-free w:tcPrChange record(s) with a prior vAlign, solid direct-color shd, tcMar, tcW, noWrap, textDirection, tcFitText, hideMark, cnfStyle, hMerge, vMerge, gridSpan, and/or direct-color tcBorders snapshot remain reviewable in Work as cell-formatting changes and round-trip as native w:tcPrChange records.`,
            'info',
          ),
        );
      }
      if (supportedNumberingRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.numbering',
            'Numbering revisions',
            `${supportedNumberingRevisionCount} bounded ordered-list and bullet-list numbering revision(s) preserve author, date, prior start, and common decimal, letter, Roman, or bullet (nfc 23) formats at the current level, including bounded multi-level originals whose sibling levels may carry other ST_NumberFormat values as opaque prior text. Contiguous list-item records remain reviewable as one Work change and round-trip as native w:numberingChange records.`,
            'info',
          ),
        );
      }
      if (supportedParagraphMarkRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.paragraph-mark',
            'Paragraph-mark revisions',
            `${supportedParagraphMarkRevisionCount} bounded paragraph-mark insertion/deletion revision(s) preserve author, date, and whole-paragraph accept/reject semantics through Work and native DOCX w:pPr/w:rPr/w:ins or w:del round trips, including multi-wrapper text-only bodies that share the mark author and date plus soft breaks, tabs, carriage returns, last-rendered page breaks, page-number and date-field glyphs, footnoteRef, endnoteRef, annotationRef, separator, and continuationSeparator glyphs, non-breaking and soft hyphens, relationship-free internal hyperlinks, safe relationship-bound external hyperlinks, relationship-free bookmarks, supported inline DrawingML pictures (inside wrappers, as untracked siblings, and picture-only bodies), and untracked text-only sibling runs (including empty/rPr-only).`,
            'info',
          ),
        );
      }
      if (isolatedParagraphBreakMarkRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.paragraph-break',
            'Paragraph-break revisions',
            `${isolatedParagraphBreakMarkRevisionCount} isolated paragraph-mark revision(s) look like paragraph-break merge or split candidates. Eligible text-only neighbor pairs (including soft breaks, tabs, carriage returns, last-rendered page breaks, page-number and date-field glyphs, footnoteRef, endnoteRef, annotationRef, separator, and continuationSeparator glyphs, non-breaking and soft hyphens, relationship-free internal hyperlinks, safe relationship-bound external hyperlinks, relationship-free bookmarks, supported inline DrawingML pictures including picture-only bodies, and empty/rPr-only runs) become reviewable paragraph-break changes; others stay fail-closed diagnostics instead of whole-paragraph guesses.`,
          ),
        );
      }
      if (supportedMovePairCount) {
        issues.push(
          issue(
            'docx.revisions.move',
            'Move revisions',
            `${supportedMovePairCount} bounded text move revision(s) preserve author, date, source and destination text, remain reviewable as one atomic Work change, and round-trip as native w:moveFrom and w:moveTo records${
              moveRangeCompanions.companionCount
                ? ` with ${moveRangeCompanions.companionCount} companion w:move*Range* bookmark set(s)`
                : ''
            }. Rich content, unpaired range markers, and relationship-bound moves remain on the compatibility path.`,
            'info',
          ),
        );
      }
      if (moveRangeCompanions.unpairedMarkerCount) {
        issues.push(
          issue(
            'docx.revisions.move-range',
            'Move-range revisions',
            `${moveRangeCompanions.unpairedMarkerCount} w:move*Range* marker(s) are not companions of a supported text-only moveFrom/moveTo pair. Nested-table, SDT, section-sandwich, and tracked or rich sibling-cell ranges, plus unpaired markers, stay fail-closed.`,
          ),
        );
      }
      if (preservableTablePropertyRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.table-property',
            'Table-property revisions',
            `${preservableTablePropertyRevisionCount} relationship-free w:tblPrChange record(s) that are outside the reviewable alignment/preferred-width/indent/cell-margin/layout/bidiVisual/solid-fill/tblLook/tblOverlap/tblStyle/tblCellSpacing/tblStyleColBandSize/tblStyleRowBandSize/tblBorders/tblCaption/tblDescription/tblpPr subset round-trip as opaque table metadata on untouched tables. They are not yet reviewable in Work; relationship-bound or spoofed records stay fail-closed. Alignment, preferred-width, indent, default cell-margin, layout, bidiVisual, solid-fill, tblLook, tblOverlap, relationship-free tblStyle, dxa tblCellSpacing, relationship-free tblStyleColBandSize, relationship-free tblStyleRowBandSize, direct-color tblBorders, relationship-free tblCaption, relationship-free tblDescription, and/or relationship-free attribute-only tblpPr records are reported separately as reviewable table-formatting revisions.`,
            'info',
          ),
        );
      }
      if (preservableRowPropertyRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.row-property',
            'Row-property revisions',
            `${preservableRowPropertyRevisionCount} relationship-free w:trPrChange record(s) outside the reviewable cantSplit/tblHeader/trHeight/hidden/jc/gridBefore/gridAfter/wBefore/wAfter/cnfStyle/divId/tblCellSpacing subset round-trip as opaque row metadata on untouched rows. They are not yet reviewable in Work; relationship-bound or spoofed records stay fail-closed. cantSplit, tblHeader, trHeight, hidden, jc, gridBefore, gridAfter, wBefore, wAfter, cnfStyle, divId, and/or tblCellSpacing records are reported separately as reviewable row-formatting revisions.`,
            'info',
          ),
        );
      }
      if (preservableCellPropertyRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.cell-property',
            'Cell-property revisions',
            `${preservableCellPropertyRevisionCount} relationship-free w:tcPrChange record(s) outside the reviewable vAlign/solid-shd/tcMar/tcW/noWrap/textDirection/tcFitText/hideMark/cnfStyle/hMerge/vMerge/gridSpan/tcBorders subset round-trip as opaque cell metadata on untouched cells. They are not yet reviewable in Work; relationship-bound or spoofed records stay fail-closed. vAlign, solid-shd, tcMar, tcW, noWrap, textDirection, tcFitText, hideMark, cnfStyle, hMerge, vMerge, gridSpan, and/or tcBorders records are reported separately as reviewable cell-formatting revisions.`,
            'info',
          ),
        );
      }
      const sectionPropertyRevisions = descendants(document, 'sectPrChange');
      const supportedSectionFormattingRevisionCount =
        sectionPropertyRevisions.filter(
          isSupportedDocxSectionFormattingChange,
        ).length;
      const preservableSectionPropertyRevisionCount =
        sectionPropertyRevisions.filter((change) => {
          if (isSupportedDocxSectionFormattingChange(change)) return false;
          const properties = change.parentElement;
          if (properties?.localName !== 'sectPr') return false;
          return (
            serializePreservableDocxSectionPropertyRevision(properties) !== null
          );
        }).length;
      const unpreservableSectionPropertyRevisionCount =
        sectionPropertyRevisions.length -
        preservableSectionPropertyRevisionCount -
        supportedSectionFormattingRevisionCount;
      if (preservableSectionPropertyRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.section-property',
            'Section-property revisions',
            `${preservableSectionPropertyRevisionCount} relationship-free w:sectPrChange record(s) outside the reviewable orientation/page-geometry/page-margin/paper-source/equal-width-cols/unequal-width-cols/titlePg/rtlGutter/docGrid/lnNumType/pgNumType/formProt/vAlign/noEndnote/textDirection/bidi/footnotePr/endnotePr/type/pgBorders subset round-trip as opaque section metadata on untouched sections. They are not yet reviewable in Work; relationship-bound or spoofed records stay fail-closed. Orientation, full page-geometry, page-margin, paper-source, equal-width column, unequal-width column, titlePg, rtlGutter, docGrid, lnNumType, pgNumType, formProt, vAlign, noEndnote, textDirection, bidi, footnotePr, endnotePr, type, and/or pgBorders records are reported separately as reviewable section-formatting revisions.`,
            'info',
          ),
        );
      }
      if (supportedSectionFormattingRevisionCount) {
        issues.push(
          issue(
            'docx.revisions.section-formatting',
            'Section-formatting revisions',
            `${supportedSectionFormattingRevisionCount} relationship-free w:sectPrChange record(s) with a prior orientation-only or complete w:pgSz, complete w:pgMar, w:paperSrc, equal-width or unequal-width w:cols, w:titlePg, w:rtlGutter, bounded w:docGrid, bounded w:lnNumType, bounded w:pgNumType, w:formProt, w:vAlign, w:noEndnote, w:textDirection, w:bidi, bounded w:footnotePr, bounded w:endnotePr, w:type, and/or bounded w:pgBorders snapshot remain reviewable in Work as section-formatting changes and round-trip as native w:sectPrChange records.`,
            'info',
          ),
        );
      }
      if (
        textRevisions.some(
          (revision) =>
            !supportedParagraphMarkRevisions.includes(revision) &&
            !isolatedParagraphBreakMarkRevisions.includes(revision) &&
            !descendants(revision, 't').length &&
            !descendants(revision, 'delText').length,
        ) ||
        supportedRunFormattingRevisionCount !== runFormattingRevisions.length ||
        supportedParagraphFormattingRevisionCount !==
          paragraphFormattingRevisions.length ||
        supportedParagraphMarkRevisionCount +
          isolatedParagraphBreakMarkRevisionCount !==
          paragraphMarkRevisions.length ||
        supportedNumberingRevisionCount !== numberingRevisions.length ||
        supportedMoveRevisionCount !== moveRevisionCount ||
        supportedMoveRevisionCount !== supportedMovePairCount * 2 ||
        unpreservableTablePropertyRevisionCount > 0 ||
        unpreservableRowPropertyRevisionCount > 0 ||
        unpreservableCellPropertyRevisionCount > 0 ||
        unpreservableSectionPropertyRevisionCount > 0
      ) {
        issues.push(
          issue(
            'docx.revisions.structural',
            'Structural revisions',
            'Moved content plus unsupported character formatting, paragraph formatting, numbering, and unpreservable table/row/cell/section-property revisions may be normalized; Work currently reviews body-text insertions/deletions and bounded whole-paragraph mark, text-move, character-, paragraph-, table-formatting (alignment/preferred-width/indent/cell margins/layout/bidiVisual/solid fill/tblLook/tblOverlap/tblStyle/tblCellSpacing/tblStyleColBandSize/tblStyleRowBandSize/tblBorders/tblCaption/tblDescription/tblpPr), row-formatting (cantSplit/tblHeader/trHeight/hidden/jc/gridBefore/gridAfter/wBefore/wAfter/cnfStyle/divId/tblCellSpacing), cell-formatting (vAlign/solid fill/tcMar/tcW/noWrap/textDirection/tcFitText/hideMark/cnfStyle/hMerge/vMerge/gridSpan/tcBorders), section-formatting (orientation/page geometry/page margins/paper source/equal-width or unequal-width columns/titlePg/rtlGutter/docGrid/lnNumType/pgNumType/formProt/vAlign/noEndnote/textDirection/bidi/footnotePr/endnotePr/type/pgBorders), and ordered-list-numbering subsets, including live track-changes for those formatting families. Relationship-free non-reviewable w:tblPrChange plus broader w:trPrChange, w:tcPrChange, and w:sectPrChange records are reported separately. Isolated paragraph-break mark revisions are reported separately as docx.revisions.paragraph-break. Unpaired w:move*Range* markers are reported separately as docx.revisions.move-range.',
          ),
        );
      }
      if (captionDiagnostics.hasUnsupportedFields) {
        issues.push(
          issue(
            'docx.fields',
            'Fields',
            'Fields beyond supported body fields, citations, bibliographies, caption SEQ fields, and bookmark or caption REF fields are converted to their current displayed value.',
          ),
        );
      }
      const sectionProperties = descendants(document, 'sectPr');
      const columnProperties = descendants(document, 'cols');
      if (sectionProperties.length > 1 || columnProperties.length) {
        issues.push(
          issue(
            'docx.sections',
            'Sections and columns',
            `${sectionProperties.length || 1} section(s) and equal or custom-width column settings are preserved, editable, and applied to document PDF output.`,
            'info',
          ),
        );
      }
      const incompleteUnequalColumns = columnProperties.some(
        (columns) =>
          (attribute(columns, 'equalWidth') === '0' ||
            attribute(columns, 'equalWidth') === 'false') &&
          directChildren(columns, 'col').length === 0,
      );
      const excessiveColumns = columnProperties.some(
        (columns) => Number(attribute(columns, 'num')) > 6,
      );
      if (incompleteUnequalColumns || excessiveColumns) {
        issues.push(
          issue(
            'docx.sections.unsupported',
            'Sections and columns',
            `${[
              incompleteUnequalColumns
                ? 'custom columns without explicit width definitions'
                : '',
              excessiveColumns ? 'sections with more than six columns' : '',
            ]
              .filter(Boolean)
              .join(' and ')} are normalized to at most six editable columns.`,
          ),
        );
      }
      if (
        descendants(document, 'type').some(
          (sectionType) =>
            sectionType.parentElement?.localName === 'sectPr' &&
            attribute(sectionType, 'val') === 'nextColumn',
        )
      ) {
        issues.push(
          issue(
            'docx.sections.next-column-preview',
            'Sections and columns',
            'Next-column section breaks survive DOCX round-trips; Work preview and PDF render them as continuous section blocks.',
          ),
        );
      }
    }
  } catch {
    issues.push(
      issue(
        'docx.inspect',
        'Package inspection',
        'Some DOCX features could not be inspected before conversion.',
      ),
    );
  }

  return report(file, 'DOCX', issues);
}


function docxUnsupportedImageTransformCount(document: Document): number {
  const drawings = [
    ...descendants(document, 'anchor'),
    ...descendants(document, 'inline'),
  ];
  return drawings.filter(
    (drawing) =>
      descendants(drawing, 'pic').length > 0 &&
      !readDocxImageTransform(drawing).supported,
  ).length;
}

function report(
  file: File,
  sourceFormat: string,
  issues: WorkCompatibilityIssue[],
): WorkCompatibilityReport {
  return {
    sourceFormat,
    sourceName: file.name,
    assessedAt: Date.now(),
    issues: deduplicate(issues),
  };
}

function issue(
  code: string,
  feature: string,
  message: string,
  severity: WorkCompatibilityIssue['severity'] = 'warning',
  location?: string,
): WorkCompatibilityIssue {
  return { code, feature, message, severity, location };
}

function deduplicate(
  issues: WorkCompatibilityIssue[],
): WorkCompatibilityIssue[] {
  const seen = new Set<string>();
  return issues.filter((item) => {
    const key = `${item.code}:${item.location ?? ''}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
