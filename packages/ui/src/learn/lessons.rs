//! Lesson content. Each lesson is a few blocks of text, optional
//! calculators, links to where the idea shows up in the app, and one
//! question to check it landed.

use super::tools::Tool;
use crate::{dashboard::Tab as PortfolioTab, i18n::Lang, stock_page::Tab as StockTab};

/// Text in both languages. Lessons keep the Thai next to the English
/// instead of in the `tr` dictionary: the paragraphs are long, and a
/// dictionary keyed by the English would silently drop the Thai whenever
/// the English is edited.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct T {
    pub en: &'static str,
    pub th: &'static str,
}

impl T {
    /// The text in the current language.
    pub fn get(self) -> &'static str {
        match crate::i18n::current() {
            Lang::En => self.en,
            Lang::Th => self.th,
        }
    }
}

const fn t(en: &'static str, th: &'static str) -> T {
    T { en, th }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Track {
    Stocks,
    Risk,
}

impl Track {
    pub const ALL: [Self; 2] = [Self::Stocks, Self::Risk];

    pub fn title(self) -> T {
        match self {
            Self::Stocks => t("Analysing a stock", "วิเคราะห์หุ้น"),
            Self::Risk => t("Understanding risk", "เข้าใจความเสี่ยง"),
        }
    }

    pub fn blurb(self) -> T {
        match self {
            Self::Stocks => t(
                "From what the company does to whether its price makes sense.",
                "ตั้งแต่บริษัททำธุรกิจอะไร ไปจนถึงราคาตอนนี้สมเหตุสมผลหรือเปล่า",
            ),
            Self::Risk => t(
                "What the numbers on your Risk tab mean, and how to keep losses bearable.",
                "ตัวเลขในแท็บความเสี่ยงหมายถึงอะไร และจะคุมการขาดทุนให้รับไหวได้อย่างไร",
            ),
        }
    }

    /// This track's lessons in order, with their index in [`LESSONS`].
    pub fn lessons(self) -> impl Iterator<Item = (usize, &'static Lesson)> {
        LESSONS.iter().enumerate().filter(move |(_, l)| l.track == self)
    }
}

pub enum Block {
    P(T),
    /// A subheading.
    H(T),
    List(&'static [T]),
    /// Term and its explanation.
    Terms(&'static [(T, T)]),
    /// A formula and a line on how to read it.
    Formula(T, T),
    Tip(T),
    Warn(T),
}

/// Where a "see it in the app" link goes.
pub enum Go {
    Page(&'static str),
    Portfolio(PortfolioTab),
    /// An example stock's page, on a tab.
    Stock(&'static str, StockTab),
}

pub struct Quiz {
    pub question: T,
    pub options: &'static [T],
    pub answer: usize,
    pub why: T,
}

pub struct Lesson {
    /// URL segment: `/learn/{slug}`.
    pub slug: &'static str,
    pub track: Track,
    pub title: T,
    pub summary: T,
    pub minutes: u8,
    pub body: &'static [Block],
    pub tools: &'static [Tool],
    pub see: &'static [(T, Go)],
    pub quiz: Quiz,
}

pub fn find(slug: &str) -> Option<usize> {
    LESSONS.iter().position(|l| l.slug == slug)
}

pub static LESSONS: &[Lesson] = &[
    // ─── Analysing a stock ───────────────────────────────────────────────────
    Lesson {
        slug: "business",
        track: Track::Stocks,
        title: t("Start with the business", "เริ่มจากเข้าใจธุรกิจ"),
        summary: t(
            "What the company sells, why customers stay, and what could go wrong.",
            "บริษัทขายอะไร ทำไมลูกค้าถึงอยู่ด้วย และอะไรที่อาจพังได้",
        ),
        minutes: 4,
        body: &[
            Block::P(t(
                "A share is a small piece of a real company. Before looking at any ratio, you should be able to say in two sentences what the company sells, who pays for it, and why those customers don't simply switch to a rival.",
                "หุ้นคือส่วนเล็กๆ ของบริษัทจริงๆ ก่อนจะดูอัตราส่วนใดๆ คุณควรอธิบายได้ในสองประโยคว่าบริษัทขายอะไร ใครเป็นคนจ่ายเงิน และทำไมลูกค้าเหล่านั้นถึงไม่ย้ายไปใช้ของคู่แข่ง",
            )),
            Block::H(t("Five questions to answer", "5 คำถามที่ต้องตอบให้ได้")),
            Block::List(&[
                t(
                    "How does it make money? Its products and customers, and whether revenue repeats (subscriptions, contracts) or has to be won again every time.",
                    "ทำเงินจากอะไร? สินค้าและลูกค้า และรายได้เกิดซ้ำได้เอง (สมาชิกรายเดือน สัญญาระยะยาว) หรือต้องหาลูกค้าใหม่ทุกครั้ง",
                ),
                t(
                    "Why is it growing? A growing market, winning customers from rivals, or raising prices — and can that go on?",
                    "ทำไมถึงเติบโต? ตลาดโตขึ้น แย่งลูกค้าจากคู่แข่ง หรือขึ้นราคาได้ และจะไปต่อได้อีกนานไหม",
                ),
                t(
                    "What protects it (its moat)? A brand, network effects, high switching costs, lower costs than rivals, patents or licences.",
                    "อะไรปกป้องธุรกิจ (คูเมือง หรือ moat)? แบรนด์ เครือข่ายผู้ใช้ ต้นทุนการเปลี่ยนเจ้าที่สูง ต้นทุนต่ำกว่าคู่แข่ง สิทธิบัตรหรือใบอนุญาต",
                ),
                t(
                    "Who runs it? Their track record, whether they own shares themselves, and what they spend the company's cash on.",
                    "ใครบริหาร? ผลงานที่ผ่านมา ผู้บริหารถือหุ้นเองหรือไม่ และใช้เงินของบริษัทไปกับอะไร",
                ),
                t(
                    "What could break it? New technology, regulation, depending on one big customer or supplier, or too much debt.",
                    "อะไรทำให้พังได้? เทคโนโลยีใหม่ กฎหมาย การพึ่งพาลูกค้าหรือซัพพลายเออร์รายใหญ่รายเดียว หรือหนี้ที่มากเกินไป",
                ),
            ]),
            Block::Tip(t(
                "If you can't explain how a company makes money, you can't tell whether its price is too high. Not understanding the business is a risk in itself.",
                "ถ้าอธิบายไม่ได้ว่าบริษัททำเงินจากอะไร ก็ตัดสินไม่ได้ว่าราคาแพงเกินไปหรือเปล่า การไม่เข้าใจธุรกิจก็เป็นความเสี่ยงอย่างหนึ่ง",
            )),
        ],
        tools: &[],
        see: &[
            (t("A stock's Summary: About and Peers", "หน้าหุ้น แท็บสรุป: ข้อมูลบริษัทและคู่แข่ง"), Go::Stock("AAPL", StockTab::Summary)),
            (t("A stock's News", "หน้าหุ้น แท็บข่าว"), Go::Stock("AAPL", StockTab::News)),
        ],
        quiz: Quiz {
            question: t("Which of these is a moat?", "ข้อใดคือ \"คูเมือง\" (moat) ของธุรกิจ?"),
            options: &[
                t("The share price rose 40% last year", "ราคาหุ้นขึ้น 40% เมื่อปีที่แล้ว"),
                t(
                    "Customers would lose their data and weeks of work if they switched to a rival",
                    "ลูกค้าจะเสียข้อมูลและเวลาหลายสัปดาห์ถ้าย้ายไปใช้ของคู่แข่ง",
                ),
                t("Most analysts rate it Strong buy", "นักวิเคราะห์ส่วนใหญ่ให้ Strong buy"),
            ],
            answer: 1,
            why: t(
                "A moat is a lasting business advantage — here, high switching costs. Past price moves and analyst ratings say nothing about how well the business is protected.",
                "moat คือความได้เปรียบทางธุรกิจที่อยู่ได้นาน ในข้อนี้คือต้นทุนการเปลี่ยนเจ้าที่สูง ส่วนราคาหุ้นที่ผ่านมาและเรตติ้งนักวิเคราะห์ไม่ได้บอกว่าธุรกิจถูกปกป้องดีแค่ไหน",
            ),
        },
    },
    Lesson {
        slug: "financials",
        track: Track::Stocks,
        title: t("Read the financial statements", "อ่านงบการเงิน"),
        summary: t(
            "Income statement, balance sheet and cash flow: are the numbers backing up the story?",
            "งบกำไรขาดทุน งบดุล และงบกระแสเงินสด: ตัวเลขรองรับเรื่องที่บริษัทเล่าไหม",
        ),
        minutes: 6,
        body: &[
            Block::P(t(
                "Three statements show whether the story is true. The income statement shows sales and profit over a period, the balance sheet what the company owns and owes on one day, and the cash flow statement the real cash that came in and went out.",
                "งบการเงินสามงบบอกว่าเรื่องที่บริษัทเล่าเป็นจริงไหม งบกำไรขาดทุนแสดงยอดขายและกำไรในช่วงเวลาหนึ่ง งบดุลแสดงทรัพย์สินและหนี้สินของบริษัท ณ วันหนึ่ง และงบกระแสเงินสดแสดงเงินสดที่เข้าและออกจริง",
            )),
            Block::Terms(&[
                (
                    t("Revenue", "รายได้ (Revenue)"),
                    t(
                        "Money from sales, before any costs. The top line.",
                        "เงินจากการขายก่อนหักต้นทุนใดๆ เป็นบรรทัดบนสุดของงบ",
                    ),
                ),
                (
                    t("Gross and operating margin", "อัตรากำไรขั้นต้นและจากการดำเนินงาน"),
                    t(
                        "Profit left from each 100 of sales after the cost of the goods (gross) or after all running costs (operating). Steady or rising margins suggest pricing power.",
                        "กำไรที่เหลือจากยอดขายทุก 100 หลังหักต้นทุนสินค้า (ขั้นต้น) หรือหลังหักค่าใช้จ่ายดำเนินงานทั้งหมด (จากการดำเนินงาน) margin ที่คงที่หรือเพิ่มขึ้นบ่งบอกว่ามีอำนาจตั้งราคา",
                    ),
                ),
                (
                    t("Net income and EPS", "กำไรสุทธิ และ EPS"),
                    t(
                        "Profit after everything, including interest and tax. EPS is net income per share.",
                        "กำไรหลังหักทุกอย่าง รวมดอกเบี้ยและภาษี EPS คือกำไรสุทธิต่อหุ้น",
                    ),
                ),
                (
                    t("Free cash flow", "กระแสเงินสดอิสระ (Free cash flow)"),
                    t(
                        "Operating cash flow minus capital spending: cash the company can pay out, use to cut debt, or reinvest.",
                        "กระแสเงินสดจากการดำเนินงานหักรายจ่ายลงทุน คือเงินสดที่บริษัทนำไปจ่ายปันผล ลดหนี้ หรือลงทุนต่อได้จริง",
                    ),
                ),
                (
                    t("Debt-to-equity", "หนี้สินต่อทุน (D/E)"),
                    t(
                        "Debt compared with the owners' money. High debt makes profits swing harder and can sink a company in a bad year.",
                        "หนี้เทียบกับเงินของผู้ถือหุ้น หนี้สูงทำให้กำไรแกว่งแรงขึ้น และอาจทำให้บริษัทล้มได้ในปีที่แย่",
                    ),
                ),
            ]),
            Block::H(t("What good looks like", "งบที่ดีหน้าตาเป็นอย่างไร")),
            Block::List(&[
                t(
                    "Revenue and EPS growing over several years, not just one quarter",
                    "รายได้และ EPS เติบโตต่อเนื่องหลายปี ไม่ใช่แค่ไตรมาสเดียว",
                ),
                t("Margins steady or rising", "margin คงที่หรือเพิ่มขึ้น"),
                t(
                    "Free cash flow positive and roughly following net income",
                    "กระแสเงินสดอิสระเป็นบวก และเดินไปทางเดียวกับกำไรสุทธิ",
                ),
                t(
                    "Debt that cash flow could pay off in a few years",
                    "หนี้ที่กระแสเงินสดจ่ายคืนได้ภายในไม่กี่ปี",
                ),
                t(
                    "Share count flat or falling (buybacks), not creeping up (dilution)",
                    "จำนวนหุ้นคงที่หรือลดลง (ซื้อหุ้นคืน) ไม่ใช่เพิ่มขึ้นเรื่อยๆ (dilution)",
                ),
            ]),
            Block::Warn(t(
                "Red flags: profit rising while operating cash flow falls, debt growing faster than revenue, \"one-off\" charges every year, and a share count that rises year after year.",
                "สัญญาณอันตราย: กำไรเพิ่มแต่กระแสเงินสดจากการดำเนินงานลดลง หนี้โตเร็วกว่ารายได้ มีรายการ \"พิเศษครั้งเดียว\" ทุกปี และจำนวนหุ้นเพิ่มขึ้นทุกปี",
            )),
        ],
        tools: &[],
        see: &[(
            t("A stock's Financials: revenue, margins, cash flow, balance sheet", "หน้าหุ้น แท็บงบการเงิน: รายได้ margin กระแสเงินสด งบดุล"),
            Go::Stock("MSFT", StockTab::Financials),
        )],
        quiz: Quiz {
            question: t(
                "Net income has risen for three years, but operating cash flow has fallen each year. What's the sensible reaction?",
                "กำไรสุทธิเพิ่มขึ้นสามปีติด แต่กระแสเงินสดจากการดำเนินงานลดลงทุกปี ควรทำอย่างไร?",
            ),
            options: &[
                t("Ignore it — profit is what counts", "ไม่ต้องสนใจ กำไรสำคัญที่สุด"),
                t(
                    "Dig in: find out why profit isn't turning into cash",
                    "ขุดดูให้ลึก ว่าทำไมกำไรไม่กลายเป็นเงินสด",
                ),
                t("Buy more — rising profit means it's cheap", "ซื้อเพิ่ม กำไรเพิ่มแปลว่าหุ้นถูก"),
            ],
            answer: 1,
            why: t(
                "Accounting profit can run ahead of cash — for example, sales booked before customers pay. Cash is much harder to dress up, so a widening gap needs an explanation.",
                "กำไรทางบัญชีมาก่อนเงินสดได้ เช่น บันทึกยอดขายก่อนลูกค้าจ่ายเงิน แต่เงินสดแต่งตัวเลขได้ยากกว่ามาก ช่องว่างที่ถ่างออกจึงต้องหาคำอธิบายให้ได้",
            ),
        },
    },
    Lesson {
        slug: "valuation",
        track: Track::Stocks,
        title: t("Is the price reasonable?", "ราคาแพงหรือถูก?"),
        summary: t(
            "P/E, PEG, P/B, EV multiples and dividend yield — and how to compare them fairly.",
            "P/E, PEG, P/B, EV multiple และอัตราปันผล และวิธีเปรียบเทียบให้ถูกคู่",
        ),
        minutes: 6,
        body: &[
            Block::P(t(
                "A great company can still be a poor investment if you pay too much. Valuation ratios compare the price with what you get for it.",
                "บริษัทที่ดีมากก็อาจเป็นการลงทุนที่แย่ได้ถ้าจ่ายแพงเกินไป อัตราส่วนมูลค่า (valuation) เปรียบเทียบราคาที่จ่ายกับสิ่งที่ได้รับ",
            )),
            Block::Terms(&[
                (
                    t("P/E", "P/E"),
                    t(
                        "Price ÷ earnings per share: how many years of today's profit you're paying for. Forward P/E uses analysts' estimate of next year's EPS.",
                        "ราคา ÷ กำไรต่อหุ้น คือจ่ายเท่ากับกำไรปัจจุบันกี่ปี ส่วน Forward P/E ใช้ EPS ปีหน้าที่นักวิเคราะห์ประมาณการ",
                    ),
                ),
                (
                    t("PEG", "PEG"),
                    t(
                        "P/E ÷ yearly EPS growth (in %). Around 1 is often called fair for a growing company; well above 2 means you're paying a lot for the growth.",
                        "P/E ÷ อัตราการเติบโตของ EPS ต่อปี (เป็น %) ประมาณ 1 มักถือว่าสมเหตุสมผลสำหรับบริษัทที่กำลังโต ถ้าเกิน 2 มากแปลว่าจ่ายแพงเพื่อการเติบโต",
                    ),
                ),
                (
                    t("P/B", "P/B"),
                    t(
                        "Price ÷ book value (assets minus debts) per share. Most useful for banks, insurers and asset-heavy firms.",
                        "ราคา ÷ มูลค่าทางบัญชี (สินทรัพย์หักหนี้สิน) ต่อหุ้น มีประโยชน์ที่สุดกับธนาคาร ประกัน และธุรกิจที่มีสินทรัพย์มาก",
                    ),
                ),
                (
                    t("P/S", "P/S"),
                    t(
                        "Price ÷ sales per share. For companies that don't make a profit yet.",
                        "ราคา ÷ ยอดขายต่อหุ้น ใช้กับบริษัทที่ยังไม่มีกำไร",
                    ),
                ),
                (
                    t("EV/EBITDA, EV/revenue", "EV/EBITDA, EV/revenue"),
                    t(
                        "Whole-company value (market cap plus debt, minus cash) ÷ operating profit before depreciation, or ÷ sales. Fairer than P/E between companies with very different debt.",
                        "มูลค่ากิจการ (มาร์เก็ตแคป + หนี้ − เงินสด) ÷ กำไรจากการดำเนินงานก่อนค่าเสื่อม หรือ ÷ ยอดขาย เปรียบเทียบบริษัทที่มีหนี้ต่างกันมากได้ยุติธรรมกว่า P/E",
                    ),
                ),
                (
                    t("Dividend yield", "อัตราเงินปันผล"),
                    t(
                        "Yearly dividend ÷ price. An unusually high yield often means the market expects the dividend to be cut.",
                        "เงินปันผลต่อปี ÷ ราคา ถ้าสูงผิดปกติ มักแปลว่าตลาดคาดว่าจะมีการลดปันผล",
                    ),
                ),
            ]),
            Block::Formula(
                t("Earnings yield = 1 ÷ P/E", "Earnings yield = 1 ÷ P/E"),
                t(
                    "A P/E of 25 is a 4% earnings yield. Compare it with the risk-free rate: if a government bond pays about the same, you're taking stock risk for no extra return — unless profits grow.",
                    "P/E 25 เท่ากับ earnings yield 4% ลองเทียบกับอัตราผลตอบแทนไร้ความเสี่ยง ถ้าพันธบัตรรัฐบาลให้พอๆ กัน แปลว่าคุณรับความเสี่ยงหุ้นโดยไม่ได้ผลตอบแทนเพิ่ม เว้นแต่กำไรจะเติบโต",
                ),
            ),
            Block::H(t("Compare like with like", "เปรียบเทียบให้ถูกคู่")),
            Block::List(&[
                t(
                    "Against peers in the same industry: a bank at P/E 10 and a software company at P/E 30 can both be fairly priced.",
                    "เทียบกับคู่แข่งในอุตสาหกรรมเดียวกัน ธนาคารที่ P/E 10 กับบริษัทซอฟต์แวร์ที่ P/E 30 อาจราคาเหมาะสมทั้งคู่",
                ),
                t(
                    "Against the stock's own history: is it cheaper or dearer than usual, and why?",
                    "เทียบกับอดีตของหุ้นตัวเอง ตอนนี้ถูกหรือแพงกว่าปกติ และเพราะอะไร",
                ),
                t(
                    "A low P/E can be a value trap if profits are about to fall.",
                    "P/E ต่ำอาจเป็นกับดัก (value trap) ถ้ากำไรกำลังจะลดลง",
                ),
                t(
                    "With tiny or negative earnings P/E means nothing — use P/S or EV/revenue instead.",
                    "ถ้ากำไรน้อยมากหรือขาดทุน P/E ไม่มีความหมาย ให้ใช้ P/S หรือ EV/revenue แทน",
                ),
            ]),
        ],
        tools: &[Tool::Valuation],
        see: &[
            (t("A stock's Statistics: valuation now and each quarter", "หน้าหุ้น แท็บสถิติ: valuation ตอนนี้และทุกไตรมาส"), Go::Stock("AAPL", StockTab::Statistics)),
            (t("A stock's Compare: side by side with peers", "หน้าหุ้น แท็บเปรียบเทียบ: เทียบกับคู่แข่ง"), Go::Stock("AAPL", StockTab::Compare)),
        ],
        quiz: Quiz {
            question: t(
                "Stock A: P/E 30, EPS growing 30% a year. Stock B: P/E 15, EPS growing 5% a year. Which has the lower PEG?",
                "หุ้น A: P/E 30, EPS โตปีละ 30% หุ้น B: P/E 15, EPS โตปีละ 5% ตัวไหนมี PEG ต่ำกว่า?",
            ),
            options: &[t("Stock A", "หุ้น A"), t("Stock B", "หุ้น B"), t("The same", "เท่ากัน")],
            answer: 0,
            why: t(
                "PEG = P/E ÷ growth. A: 30 ÷ 30 = 1.0. B: 15 ÷ 5 = 3.0. The stock with the \"expensive\" P/E is cheaper once growth is counted — as long as the growth really happens.",
                "PEG = P/E ÷ การเติบโต A: 30 ÷ 30 = 1.0, B: 15 ÷ 5 = 3.0 หุ้นที่ P/E ดู \"แพง\" กลับถูกกว่าเมื่อนับการเติบโต ตราบใดที่การเติบโตเกิดขึ้นจริง",
            ),
        },
    },
    Lesson {
        slug: "quality",
        track: Track::Stocks,
        title: t("Quality, growth and analysts", "คุณภาพ การเติบโต และนักวิเคราะห์"),
        summary: t(
            "ROE, payout ratio, earnings surprises, and how far to trust price targets.",
            "ROE อัตราการจ่ายปันผล ผลประกอบการเทียบคาดการณ์ และควรเชื่อราคาเป้าหมายแค่ไหน",
        ),
        minutes: 5,
        body: &[
            Block::P(t(
                "Cheap isn't enough: you want a business that turns the owners' money into profit, again and again.",
                "ถูกอย่างเดียวไม่พอ ต้องเป็นธุรกิจที่เปลี่ยนเงินของผู้ถือหุ้นเป็นกำไรได้ซ้ำแล้วซ้ำเล่า",
            )),
            Block::Terms(&[
                (
                    t("ROE", "ROE"),
                    t(
                        "Net income ÷ shareholders' equity: profit on the owners' money. Above about 15% for years is strong — but check it isn't just borrowed money doing the work.",
                        "กำไรสุทธิ ÷ ส่วนของผู้ถือหุ้น คือกำไรที่ทำได้จากเงินของเจ้าของ ถ้าสูงกว่าราว 15% ต่อเนื่องหลายปีถือว่าแข็งแรง แต่ต้องเช็กว่าไม่ได้มาจากเงินกู้",
                    ),
                ),
                (
                    t("ROA", "ROA"),
                    t(
                        "Net income ÷ total assets. Useful for banks and asset-heavy firms, whose ROE is lifted by heavy borrowing.",
                        "กำไรสุทธิ ÷ สินทรัพย์รวม เหมาะกับธนาคารและธุรกิจที่มีสินทรัพย์มาก ซึ่ง ROE มักสูงเพราะใช้หนี้เยอะ",
                    ),
                ),
                (
                    t("EPS growth", "การเติบโตของ EPS"),
                    t(
                        "Look for steady growth over five years or more. Growth from buybacks alone (fewer shares) is weaker than growth from rising profit.",
                        "มองหาการเติบโตที่สม่ำเสมอ 5 ปีขึ้นไป การเติบโตที่มาจากการซื้อหุ้นคืนอย่างเดียว (หุ้นน้อยลง) อ่อนแอกว่าการเติบโตจากกำไรที่เพิ่มขึ้นจริง",
                    ),
                ),
                (
                    t("Payout ratio", "อัตราการจ่ายปันผล (Payout ratio)"),
                    t(
                        "Dividends ÷ earnings. Above about 80% leaves little room if profits dip (REITs pay out most of their income by design).",
                        "เงินปันผล ÷ กำไร ถ้าเกินราว 80% จะเหลือที่ว่างน้อยเมื่อกำไรลด (ยกเว้น REIT ที่ออกแบบมาให้จ่ายรายได้ส่วนใหญ่)",
                    ),
                ),
                (
                    t("Earnings surprise", "ผลประกอบการเทียบคาดการณ์"),
                    t(
                        "Reported EPS against the analysts' estimate. A habit of beating estimates is a good sign; one beat is noise.",
                        "EPS ที่ประกาศจริงเทียบกับที่นักวิเคราะห์คาด ถ้าชนะคาดการณ์เป็นประจำถือเป็นสัญญาณดี ชนะครั้งเดียวยังบอกอะไรไม่ได้",
                    ),
                ),
            ]),
            Block::H(t("Analyst ratings and price targets", "เรตติ้งและราคาเป้าหมายของนักวิเคราะห์")),
            Block::P(t(
                "They sum up professional opinion, but they lean optimistic, tend to agree with each other, and often move after the price does. Treat them as one input, and look at the gap between the lowest and highest target, not just the average.",
                "เป็นการสรุปความเห็นของมืออาชีพ แต่มักมองโลกในแง่ดี ความเห็นไปทางเดียวกัน และหลายครั้งปรับตามหลังราคา ใช้เป็นข้อมูลประกอบอย่างหนึ่ง และดูช่วงห่างระหว่างเป้าต่ำสุดกับสูงสุด ไม่ใช่แค่ค่าเฉลี่ย",
            )),
            Block::Tip(t(
                "Consistency beats one great year: steady ROE, margins and EPS growth over five years or more tell you more than any single quarter.",
                "ความสม่ำเสมอสำคัญกว่าปีเดียวที่ดีมาก ROE, margin และการเติบโตของ EPS ที่นิ่งตลอด 5 ปีขึ้นไป บอกอะไรได้มากกว่าไตรมาสใดไตรมาสหนึ่ง",
            )),
        ],
        tools: &[],
        see: &[(
            t("A stock's Analysis: ratings, price targets, EPS vs estimate", "หน้าหุ้น แท็บบทวิเคราะห์: เรตติ้ง ราคาเป้าหมาย EPS เทียบคาดการณ์"),
            Go::Stock("AAPL", StockTab::Analysis),
        )],
        quiz: Quiz {
            question: t(
                "Company X has an ROE of 40%, but its debt is five times its equity. What's the catch?",
                "บริษัท X มี ROE 40% แต่มีหนี้มากกว่าส่วนของผู้ถือหุ้น 5 เท่า มีอะไรต้องระวัง?",
            ),
            options: &[
                t("None — 40% is excellent", "ไม่มี 40% ถือว่ายอดเยี่ยม"),
                t(
                    "Heavy debt shrinks equity, which inflates ROE — and adds risk",
                    "หนี้ที่สูงทำให้ส่วนของผู้ถือหุ้นเล็ก ROE จึงดูสูงเกินจริง และเพิ่มความเสี่ยง",
                ),
                t("ROE has nothing to do with debt", "ROE ไม่เกี่ยวกับหนี้"),
            ],
            answer: 1,
            why: t(
                "ROE divides profit by equity. Borrowing more and keeping little equity makes the ratio look great while making the company fragile. Check ROA and debt as well.",
                "ROE คือกำไรหารด้วยส่วนของผู้ถือหุ้น ยิ่งกู้มากและมีทุนน้อย ตัวเลขก็ยิ่งดูดี แต่บริษัทก็เปราะบางขึ้น ควรดู ROA และหนี้ประกอบด้วย",
            ),
        },
    },
    Lesson {
        slug: "technical",
        track: Track::Stocks,
        title: t("Reading the price chart", "อ่านกราฟราคา (เทคนิคอล)"),
        summary: t(
            "Trend, moving averages, RSI, MACD, Bollinger Bands and volume.",
            "เทรนด์ เส้นค่าเฉลี่ย RSI MACD Bollinger Bands และวอลุ่ม",
        ),
        minutes: 5,
        body: &[
            Block::P(t(
                "Fundamentals help decide what to buy; the chart can help with when, and shows how other investors are behaving. Indicators describe what already happened — they don't predict.",
                "ปัจจัยพื้นฐานช่วยตัดสินว่าจะซื้ออะไร ส่วนกราฟช่วยเรื่องจังหวะ และบอกว่านักลงทุนคนอื่นกำลังทำอะไร อินดิเคเตอร์อธิบายสิ่งที่เกิดขึ้นแล้ว ไม่ได้ทำนายอนาคต",
            )),
            Block::Terms(&[
                (
                    t("Trend", "เทรนด์"),
                    t(
                        "Higher highs and higher lows make an uptrend; lower highs and lower lows a downtrend.",
                        "จุดสูงใหม่สูงขึ้นและจุดต่ำใหม่สูงขึ้นคือขาขึ้น จุดสูงต่ำลงและจุดต่ำต่ำลงคือขาลง",
                    ),
                ),
                (
                    t("SMA 50 / 200", "SMA 50 / 200"),
                    t(
                        "The average close over 50 or 200 days. Price above the 200-day line suggests a long-term uptrend. The 50 crossing above the 200 is a \"golden cross\"; crossing below, a \"death cross\".",
                        "ราคาปิดเฉลี่ย 50 หรือ 200 วัน ราคาอยู่เหนือเส้น 200 วันบ่งบอกขาขึ้นระยะยาว เส้น 50 ตัดขึ้นเหนือเส้น 200 เรียกว่า \"golden cross\" ตัดลงเรียกว่า \"death cross\"",
                    ),
                ),
                (
                    t("RSI (14)", "RSI (14)"),
                    t(
                        "Momentum from 0 to 100. Above 70 is called overbought, below 30 oversold — but in a strong trend it can stay there for weeks.",
                        "โมเมนตัมตั้งแต่ 0 ถึง 100 เกิน 70 เรียกว่า overbought ต่ำกว่า 30 เรียกว่า oversold แต่ในเทรนด์ที่แรงอาจค้างอยู่ตรงนั้นได้หลายสัปดาห์",
                    ),
                ),
                (
                    t("MACD", "MACD"),
                    t(
                        "The 12-day EMA minus the 26-day EMA, with a 9-day signal line. Crossing above the signal line means momentum is turning up.",
                        "EMA 12 วันลบ EMA 26 วัน พร้อมเส้นสัญญาณ 9 วัน ตัดขึ้นเหนือเส้นสัญญาณแปลว่าโมเมนตัมกำลังกลับเป็นขาขึ้น",
                    ),
                ),
                (
                    t("Bollinger Bands", "Bollinger Bands"),
                    t(
                        "A 20-day average ± 2 standard deviations. The bands widen when volatility rises and squeeze when it falls.",
                        "ค่าเฉลี่ย 20 วัน ± 2 ส่วนเบี่ยงเบนมาตรฐาน แถบจะกว้างขึ้นเมื่อผันผวนสูงและแคบลงเมื่อผันผวนต่ำ",
                    ),
                ),
                (
                    t("Volume", "วอลุ่ม (Volume)"),
                    t(
                        "Shares traded. A breakout on heavy volume is more convincing than one on light volume.",
                        "จำนวนหุ้นที่ซื้อขาย การทะลุแนวพร้อมวอลุ่มสูงน่าเชื่อถือกว่าตอนวอลุ่มเบาบาง",
                    ),
                ),
                (
                    t("52-week range", "ช่วงราคา 52 สัปดาห์"),
                    t(
                        "Where today's price sits between the year's low and high.",
                        "ราคาวันนี้อยู่ตรงไหนระหว่างราคาต่ำสุดและสูงสุดของปี",
                    ),
                ),
            ]),
            Block::Tip(t(
                "Look for two or more signals that agree, and decide in advance where you'd admit you're wrong — for example, a close below the 200-day average.",
                "มองหาสัญญาณสองตัวขึ้นไปที่ไปทางเดียวกัน และกำหนดล่วงหน้าว่าจุดไหนที่จะยอมรับว่าคิดผิด เช่น ราคาปิดต่ำกว่าเส้นเฉลี่ย 200 วัน",
            )),
        ],
        tools: &[],
        see: &[(
            t("A stock's chart: add SMA, Bollinger, RSI or MACD", "กราฟในหน้าหุ้น: เพิ่ม SMA, Bollinger, RSI หรือ MACD"),
            Go::Stock("NVDA", StockTab::Summary),
        )],
        quiz: Quiz {
            question: t("A stock's RSI is 78. What does that tell you?", "หุ้นตัวหนึ่งมี RSI 78 บอกอะไรเรา?"),
            options: &[
                t("It will fall tomorrow", "พรุ่งนี้ราคาจะลง"),
                t(
                    "It has risen hard recently — momentum is stretched",
                    "ราคาขึ้นแรงมาช่วงหนึ่ง โมเมนตัมตึงตัว",
                ),
                t("The company is overvalued", "บริษัทมีมูลค่าสูงเกินจริง"),
            ],
            answer: 1,
            why: t(
                "RSI only measures recent price momentum. It can stay above 70 for a long time in a strong uptrend, and it says nothing about what the company is worth.",
                "RSI วัดแค่โมเมนตัมของราคาช่วงที่ผ่านมา อาจค้างเหนือ 70 ได้นานในขาขึ้นที่แรง และไม่ได้บอกอะไรเกี่ยวกับมูลค่าของบริษัท",
            ),
        },
    },
    Lesson {
        slug: "checklist",
        track: Track::Stocks,
        title: t("Putting it together", "สรุปรวม: เช็กลิสต์ก่อนซื้อ"),
        summary: t(
            "A short checklist to run before buying any stock.",
            "เช็กลิสต์สั้นๆ ที่ควรไล่ดูก่อนซื้อหุ้นทุกตัว",
        ),
        minutes: 3,
        body: &[
            Block::P(t(
                "Run through this list before you buy. An item you can't tick isn't automatically a \"no\" — but you should know you're taking that risk.",
                "ไล่เช็กรายการนี้ก่อนซื้อ ข้อที่ติ๊กไม่ได้ไม่ได้แปลว่าห้ามซื้อเสมอไป แต่คุณควรรู้ตัวว่ากำลังรับความเสี่ยงข้อนั้นอยู่",
            )),
            Block::List(&[
                t("I can explain the business and what protects it.", "อธิบายธุรกิจและสิ่งที่ปกป้องธุรกิจได้"),
                t(
                    "Revenue, EPS and free cash flow have grown for several years.",
                    "รายได้ EPS และกระแสเงินสดอิสระเติบโตมาหลายปี",
                ),
                t("Debt is manageable.", "หนี้อยู่ในระดับที่จัดการได้"),
                t(
                    "The valuation is reasonable against peers and the stock's own history.",
                    "มูลค่าเหมาะสมเมื่อเทียบกับคู่แข่งและอดีตของหุ้นเอง",
                ),
                t(
                    "I know its risk: volatility, beta and worst past drawdown (on the stock's Summary).",
                    "รู้ความเสี่ยงของหุ้น: ความผันผวน beta และ drawdown ที่แย่ที่สุด (ในแท็บสรุปของหุ้น)",
                ),
                t(
                    "I've written down why I'm buying and what would make me sell.",
                    "เขียนไว้แล้วว่าซื้อเพราะอะไร และอะไรจะทำให้ขาย",
                ),
                t(
                    "The position size fits my plan (see the lesson on diversification and position sizing).",
                    "ขนาดการลงทุนเหมาะกับแผน (ดูบทกระจายความเสี่ยงและกำหนดขนาดการลงทุน)",
                ),
            ]),
            Block::Tip(t(
                "Write your reasons in the stock's notes and re-read them after each earnings report. Sell when the reasons stop being true, not when the price wiggles.",
                "เขียนเหตุผลที่ซื้อไว้ในบันทึกของหุ้น แล้วกลับมาอ่านทุกครั้งที่ประกาศงบ ขายเมื่อเหตุผลนั้นไม่จริงแล้ว ไม่ใช่เมื่อราคาแกว่ง",
            )),
        ],
        tools: &[],
        see: &[
            (t("Screener: find candidates", "คัดกรองหุ้น: หาหุ้นที่น่าสนใจ"), Go::Page("/screener")),
            (t("A stock's Summary: risk and your notes", "หน้าหุ้น แท็บสรุป: ความเสี่ยงและบันทึกของคุณ"), Go::Stock("AAPL", StockTab::Summary)),
        ],
        quiz: Quiz {
            question: t(
                "With this approach, which is a good reason to sell?",
                "ตามแนวทางนี้ ข้อใดเป็นเหตุผลที่ดีในการขาย?",
            ),
            options: &[
                t("The price fell 5% this week", "ราคาลง 5% ในสัปดาห์นี้"),
                t("The reason you bought no longer holds", "เหตุผลที่ซื้อไม่เป็นจริงแล้ว"),
                t("A friend sold theirs", "เพื่อนขายไปแล้ว"),
            ],
            answer: 1,
            why: t(
                "Short-term price moves are mostly noise. What matters is whether the business still matches the reasons you wrote down.",
                "ราคาที่ขยับระยะสั้นส่วนใหญ่เป็นแค่เสียงรบกวน สิ่งสำคัญคือธุรกิจยังตรงกับเหตุผลที่คุณเขียนไว้หรือเปล่า",
            ),
        },
    },
    // ─── Understanding risk ──────────────────────────────────────────────────
    Lesson {
        slug: "what-is-risk",
        track: Track::Risk,
        title: t("What risk really means", "ความเสี่ยงคืออะไรกันแน่"),
        summary: t(
            "The kinds of risk, which ones diversification removes, and which it can't.",
            "ความเสี่ยงมีกี่แบบ แบบไหนการกระจายการลงทุนช่วยได้ และแบบไหนช่วยไม่ได้",
        ),
        minutes: 4,
        body: &[
            Block::P(t(
                "Risk is the chance that things turn out differently from what you expect — above all, that you lose money, or need your money back while prices are down. Higher expected returns almost always come with more risk.",
                "ความเสี่ยงคือโอกาสที่ผลลัพธ์จะไม่เป็นอย่างที่คาด โดยเฉพาะโอกาสที่จะขาดทุน หรือต้องใช้เงินในตอนที่ราคากำลังตก ผลตอบแทนที่คาดหวังสูงขึ้นแทบทุกครั้งมาพร้อมความเสี่ยงที่สูงขึ้น",
            )),
            Block::Terms(&[
                (
                    t("Market risk", "ความเสี่ยงตลาด (Systematic)"),
                    t(
                        "The whole market falls — recessions, rate shocks, crises. Owning more stocks doesn't remove it.",
                        "ทั้งตลาดตกพร้อมกัน เช่น เศรษฐกิจถดถอย ดอกเบี้ยพุ่ง วิกฤต การถือหุ้นหลายตัวขึ้นก็ไม่ช่วยลดความเสี่ยงนี้",
                    ),
                ),
                (
                    t("Company risk", "ความเสี่ยงเฉพาะบริษัท (Unsystematic)"),
                    t(
                        "Bad news at one company: a failed product, a fraud, a lost contract. Diversification removes most of it.",
                        "ข่าวร้ายของบริษัทเดียว เช่น สินค้าล้มเหลว การทุจริต เสียสัญญาใหญ่ การกระจายการลงทุนช่วยลดความเสี่ยงนี้ได้เกือบหมด",
                    ),
                ),
                (
                    t("Concentration risk", "ความเสี่ยงจากการกระจุกตัว"),
                    t(
                        "Too much in one stock, sector or country.",
                        "ลงทุนในหุ้น อุตสาหกรรม หรือประเทศเดียวมากเกินไป",
                    ),
                ),
                (
                    t("Currency risk", "ความเสี่ยงค่าเงิน"),
                    t(
                        "Owning US stocks with baht: if the dollar weakens against the baht, you lose even when the stock doesn't move.",
                        "ซื้อหุ้นสหรัฐฯ ด้วยเงินบาท ถ้าดอลลาร์อ่อนค่าเทียบบาท คุณขาดทุนได้แม้ราคาหุ้นไม่ขยับ",
                    ),
                ),
                (
                    t("Interest-rate risk", "ความเสี่ยงอัตราดอกเบี้ย"),
                    t(
                        "Rising rates push bond prices down and often hit highly valued growth stocks hardest.",
                        "ดอกเบี้ยขาขึ้นทำให้ราคาพันธบัตรลดลง และมักกระทบหุ้นเติบโตที่มูลค่าสูงแรงที่สุด",
                    ),
                ),
                (
                    t("Liquidity risk", "ความเสี่ยงสภาพคล่อง"),
                    t(
                        "You can't sell quickly at a fair price — common with small, thinly traded stocks.",
                        "ขายไม่ได้เร็วในราคาที่เหมาะสม มักเกิดกับหุ้นเล็กที่ซื้อขายน้อย",
                    ),
                ),
                (
                    t("Behaviour risk", "ความเสี่ยงจากพฤติกรรม"),
                    t(
                        "Panic-selling at the bottom or chasing what just went up. Often the most expensive risk of all.",
                        "ขายตอนตื่นตระหนกที่จุดต่ำสุด หรือไล่ซื้อของที่เพิ่งขึ้นมา มักเป็นความเสี่ยงที่แพงที่สุด",
                    ),
                ),
            ]),
            Block::Tip(t(
                "Two questions matter most: how much could I lose, and could I hold on — financially and emotionally — if it happened?",
                "สองคำถามที่สำคัญที่สุด: เสียได้มากแค่ไหน และถ้าเกิดขึ้นจริง จะถือต่อไหวไหม ทั้งด้านการเงินและจิตใจ",
            )),
        ],
        tools: &[],
        see: &[(t("Your portfolio's Risk tab", "แท็บความเสี่ยงของพอร์ตคุณ"), Go::Portfolio(PortfolioTab::Risk))],
        quiz: Quiz {
            question: t(
                "Which risk can you mostly remove by owning 20–30 stocks across different sectors?",
                "ความเสี่ยงแบบไหนที่ลดได้เกือบหมดด้วยการถือหุ้น 20–30 ตัวในหลายอุตสาหกรรม?",
            ),
            options: &[
                t("Market risk", "ความเสี่ยงตลาด"),
                t("Company risk", "ความเสี่ยงเฉพาะบริษัท"),
                t("Currency risk", "ความเสี่ยงค่าเงิน"),
            ],
            answer: 1,
            why: t(
                "One company's bad news barely moves a well-spread portfolio. A market-wide fall hits everything at once, and holding more US stocks doesn't change your dollar exposure.",
                "ข่าวร้ายของบริษัทเดียวแทบไม่กระทบพอร์ตที่กระจายดี แต่ตลาดทั้งตลาดตกจะกระทบทุกตัวพร้อมกัน และการถือหุ้นสหรัฐฯ มากขึ้นก็ไม่ได้ลดความเสี่ยงค่าเงินดอลลาร์",
            ),
        },
    },
    Lesson {
        slug: "volatility",
        track: Track::Risk,
        title: t("Volatility: how much it swings", "ความผันผวน: แกว่งแรงแค่ไหน"),
        summary: t(
            "Standard deviation, annualising, and what 20% volatility really means.",
            "ส่วนเบี่ยงเบนมาตรฐาน การแปลงเป็นรายปี และความผันผวน 20% หมายถึงอะไรจริงๆ",
        ),
        minutes: 5,
        body: &[
            Block::P(t(
                "Volatility is the standard deviation of returns: how widely they scatter around their average. This app measures it on daily returns and turns it into a yearly figure.",
                "ความผันผวน (volatility) คือส่วนเบี่ยงเบนมาตรฐานของผลตอบแทน บอกว่าผลตอบแทนกระจายห่างจากค่าเฉลี่ยแค่ไหน แอปนี้วัดจากผลตอบแทนรายวันแล้วแปลงเป็นตัวเลขรายปี",
            )),
            Block::Formula(
                t("Yearly volatility = daily standard deviation × √252", "ความผันผวนรายปี = ส่วนเบี่ยงเบนมาตรฐานรายวัน × √252"),
                t(
                    "There are about 252 trading days in a year, so a typical daily move of 1% is roughly 16% a year.",
                    "ปีหนึ่งมีวันทำการประมาณ 252 วัน การขยับโดยทั่วไปวันละ 1% จึงเท่ากับราว 16% ต่อปี",
                ),
            ),
            Block::P(t(
                "A rough rule, if returns followed a bell curve: about 2 years in 3 end within ±1 volatility of the average, and 19 in 20 within ±2. Real markets have fatter tails — extreme days happen more often than the bell curve says.",
                "กฎคร่าวๆ ถ้าผลตอบแทนกระจายตัวแบบระฆังคว่ำ ประมาณ 2 ใน 3 ปีจะจบในช่วง ±1 เท่าของความผันผวนจากค่าเฉลี่ย และ 19 ใน 20 ปีจะอยู่ในช่วง ±2 เท่า แต่ตลาดจริงมีหางอ้วนกว่า วันที่ผันผวนสุดขั้วเกิดบ่อยกว่าที่ระฆังคว่ำบอก",
            )),
            Block::H(t("How this app grades it", "แอปนี้จัดระดับอย่างไร")),
            Block::List(&[
                t(
                    "Under 12%: low — bond-heavy or defensive mixes",
                    "ต่ำกว่า 12%: ต่ำ เช่น พอร์ตที่เน้นพันธบัตรหรือหุ้นปลอดภัย",
                ),
                t(
                    "12–22%: moderate — a broad stock index usually runs at 15–20%",
                    "12–22%: ปานกลาง ดัชนีหุ้นทั้งตลาดมักอยู่ราว 15–20%",
                ),
                t(
                    "Over 22%: high — single growth stocks are often 30–60%",
                    "เกิน 22%: สูง หุ้นเติบโตรายตัวมักอยู่ที่ 30–60%",
                ),
            ]),
            Block::Warn(t(
                "Volatility treats up and down moves alike, and it's measured on the past. A calm year doesn't make a stock safe.",
                "ความผันผวนนับการขึ้นและลงเหมือนกัน และวัดจากอดีต ปีที่นิ่งไม่ได้แปลว่าหุ้นปลอดภัย",
            )),
        ],
        tools: &[Tool::Volatility],
        see: &[(t("Risk tab → Volatility", "แท็บความเสี่ยง → ความผันผวน"), Go::Portfolio(PortfolioTab::Risk))],
        quiz: Quiz {
            question: t(
                "A stock has 40% volatility and an expected return of 8% a year. In a typical year (2 in 3), its return lands roughly between…",
                "หุ้นมีความผันผวน 40% และผลตอบแทนคาดหวัง 8% ต่อปี ในปีปกติ (2 ใน 3) ผลตอบแทนจะอยู่ประมาณช่วงไหน?",
            ),
            options: &[
                t("+4% and +12%", "+4% ถึง +12%"),
                t("−32% and +48%", "−32% ถึง +48%"),
                t("−8% and +8%", "−8% ถึง +8%"),
            ],
            answer: 1,
            why: t(
                "8% ± 40% gives −32% to +48%. High volatility means a wide range of outcomes, even when the average looks attractive.",
                "8% ± 40% ได้ช่วง −32% ถึง +48% ความผันผวนสูงแปลว่าผลลัพธ์กว้างมาก แม้ค่าเฉลี่ยจะดูน่าสนใจ",
            ),
        },
    },
    Lesson {
        slug: "drawdown",
        track: Track::Risk,
        title: t("Drawdowns and the maths of losses", "Drawdown และคณิตศาสตร์ของการขาดทุน"),
        summary: t(
            "Why a 50% loss needs a 100% gain, and what real crashes looked like.",
            "ทำไมขาดทุน 50% ต้องกำไร 100% ถึงจะคืนทุน และวิกฤตจริงหน้าตาเป็นอย่างไร",
        ),
        minutes: 5,
        body: &[
            Block::P(t(
                "A drawdown is the fall from a peak to a later low. Max drawdown is the worst such fall over a period — the loss you would really have sat through had you held all along.",
                "Drawdown คือการลดลงจากจุดสูงสุดไปยังจุดต่ำหลังจากนั้น Max drawdown คือการลดลงที่แย่ที่สุดในช่วงเวลานั้น เป็นการขาดทุนที่คุณต้องทนจริงถ้าถือมาตลอด",
            )),
            Block::Formula(
                t("Gain needed to recover = 1 ÷ (1 − loss) − 1", "กำไรที่ต้องได้เพื่อคืนทุน = 1 ÷ (1 − ขาดทุน) − 1"),
                t(
                    "−20% needs +25%, −50% needs +100%, and −80% needs +400%.",
                    "−20% ต้องกลับขึ้น +25%, −50% ต้อง +100% และ −80% ต้อง +400%",
                ),
            ),
            Block::P(t(
                "Losses and gains aren't symmetric: the deeper the hole, the harder the climb. That's why avoiding very deep drawdowns matters more than squeezing out a little extra return.",
                "การขาดทุนกับกำไรไม่สมมาตรกัน หลุมยิ่งลึก ยิ่งปีนกลับยาก การหลีกเลี่ยง drawdown ที่ลึกมากจึงสำคัญกว่าการเค้นผลตอบแทนเพิ่มอีกนิด",
            )),
            Block::H(t("Real crashes", "วิกฤตที่เกิดขึ้นจริง")),
            Block::List(&[
                t(
                    "SET index, 1994–98 (Tom Yum Kung crisis): about −88% from peak to low",
                    "ดัชนี SET ปี 1994–98 (วิกฤตต้มยำกุ้ง): ประมาณ −88% จากจุดสูงสุดถึงจุดต่ำสุด",
                ),
                t(
                    "Nasdaq, 2000–02: about −78%, and 15 years to get back to its old high",
                    "Nasdaq ปี 2000–02: ประมาณ −78% และใช้เวลา 15 ปีกว่าจะกลับไปจุดสูงเดิม",
                ),
                t(
                    "S&P 500, 2007–09: about −57%, and more than five years to recover",
                    "S&P 500 ปี 2007–09: ประมาณ −57% และใช้เวลากว่า 5 ปีกว่าจะคืนทุน",
                ),
                t(
                    "S&P 500, 2020: about −34% in five weeks, back at its high by August",
                    "S&P 500 ปี 2020: ประมาณ −34% ในห้าสัปดาห์ และกลับไปจุดสูงเดิมได้ในเดือนสิงหาคม",
                ),
            ]),
        ],
        tools: &[Tool::Recovery],
        see: &[(
            t("Risk tab → Performance (drawdown strip) and Max drawdown", "แท็บความเสี่ยง → ผลการดำเนินงาน (แถบ drawdown) และการลดลงสูงสุด"),
            Go::Portfolio(PortfolioTab::Risk),
        )],
        quiz: Quiz {
            question: t(
                "Your portfolio falls 50%. How much must it gain to get back to where it started?",
                "พอร์ตลดลง 50% ต้องกำไรกี่เปอร์เซ็นต์ถึงจะกลับไปที่เดิม?",
            ),
            options: &[t("50%", "50%"), t("75%", "75%"), t("100%", "100%")],
            answer: 2,
            why: t(
                "Half is left, so it has to double: 1 ÷ (1 − 0.5) − 1 = 100%.",
                "เหลือครึ่งเดียว จึงต้องโตเป็นสองเท่า: 1 ÷ (1 − 0.5) − 1 = 100%",
            ),
        },
    },
    Lesson {
        slug: "value-at-risk",
        track: Track::Risk,
        title: t("Value at risk: how bad is a bad day?", "Value at Risk: วันแย่ๆ แย่ได้แค่ไหน"),
        summary: t(
            "VaR, expected shortfall, and why neither is a worst case.",
            "VaR, Expected shortfall และทำไมทั้งสองตัวไม่ใช่กรณีเลวร้ายที่สุด",
        ),
        minutes: 4,
        body: &[
            Block::P(t(
                "Value at risk (95%, one day) is a loss you'd expect to exceed on about 1 day in 20. It's a threshold, not a worst case.",
                "Value at Risk (95%, หนึ่งวัน) คือระดับการขาดทุนที่คาดว่าจะแย่กว่านั้นประมาณ 1 วันใน 20 วัน เป็นเส้นแบ่ง ไม่ใช่กรณีที่เลวร้ายที่สุด",
            )),
            Block::P(t(
                "Expected shortfall (also called CVaR) is the average loss on those worst 1-in-20 days. It tells you how deep the tail goes.",
                "Expected shortfall (หรือ CVaR) คือค่าเฉลี่ยการขาดทุนในวันที่แย่ที่สุด 1 ใน 20 วันเหล่านั้น บอกว่าหางของการขาดทุนลึกแค่ไหน",
            )),
            Block::P(t(
                "The Risk tab uses historical VaR: it lines up your portfolio's real daily returns and finds the cut-off for the worst 5%. A quick estimate from volatility works too:",
                "แท็บความเสี่ยงใช้ VaR จากข้อมูลย้อนหลัง คือเรียงผลตอบแทนรายวันจริงของพอร์ต แล้วหาเส้นตัดของ 5% ที่แย่ที่สุด หรือจะประมาณเร็วๆ จากความผันผวนก็ได้:",
            )),
            Block::Formula(
                t(
                    "VaR 95% ≈ 1.65 × (yearly volatility ÷ √252) × portfolio value",
                    "VaR 95% ≈ 1.65 × (ความผันผวนรายปี ÷ √252) × มูลค่าพอร์ต",
                ),
                t(
                    "This assumes a bell curve, so it tends to understate how bad the tails are.",
                    "สูตรนี้สมมติว่ากระจายแบบระฆังคว่ำ จึงมักประเมินความรุนแรงของหางต่ำกว่าจริง",
                ),
            ),
            Block::Warn(t(
                "On the 1 day in 20 beyond VaR, the loss can be far larger. Use VaR together with the stress test and max drawdown, never on its own.",
                "ใน 1 วันจาก 20 วันที่เกิน VaR การขาดทุนอาจมากกว่านั้นมาก ใช้ VaR คู่กับการทดสอบภาวะวิกฤตและ max drawdown เสมอ อย่าดูตัวเดียว",
            )),
        ],
        tools: &[Tool::Var],
        see: &[(
            t("Risk tab → Value at risk, Expected shortfall and Stress test", "แท็บความเสี่ยง → มูลค่าความเสี่ยง ความเสียหายเฉลี่ยในกรณีเลวร้าย และทดสอบภาวะวิกฤต"),
            Go::Portfolio(PortfolioTab::Risk),
        )],
        quiz: Quiz {
            question: t(
                "Your 95% one-day VaR is $1,000. What does that mean?",
                "VaR 95% หนึ่งวันของคุณคือ 10,000 บาท หมายความว่าอะไร?",
            ),
            options: &[
                t("You can't lose more than $1,000 in a day", "ในหนึ่งวันจะขาดทุนไม่เกิน 10,000 บาท"),
                t(
                    "On about 1 day in 20, you'd lose $1,000 or more",
                    "ประมาณ 1 วันใน 20 วัน จะขาดทุน 10,000 บาทหรือมากกว่า",
                ),
                t("You'll lose exactly $1,000 every 20 days", "จะขาดทุน 10,000 บาทพอดีทุกๆ 20 วัน"),
            ],
            answer: 1,
            why: t(
                "VaR is a line that bad days cross about 5% of the time. It isn't a cap — the worst days can be much worse.",
                "VaR คือเส้นที่วันแย่ๆ จะข้ามไปประมาณ 5% ของเวลา ไม่ใช่เพดาน วันที่แย่ที่สุดอาจแย่กว่านั้นมาก",
            ),
        },
    },
    Lesson {
        slug: "beta",
        track: Track::Risk,
        title: t("Beta and correlation", "Beta และ Correlation"),
        summary: t(
            "How much a stock moves with the market, and why that matters in a sell-off.",
            "หุ้นขยับตามตลาดมากแค่ไหน และทำไมถึงสำคัญตอนตลาดขาลง",
        ),
        minutes: 5,
        body: &[
            Block::P(t(
                "Correlation (from −1 to +1) says how consistently two things move together. Beta says how big a stock's moves have been compared with the market's.",
                "Correlation (ตั้งแต่ −1 ถึง +1) บอกว่าสองสิ่งขยับไปด้วยกันสม่ำเสมอแค่ไหน ส่วน Beta บอกว่าหุ้นขยับแรงแค่ไหนเมื่อเทียบกับตลาด",
            )),
            Block::Formula(
                t(
                    "Beta = correlation × (stock volatility ÷ market volatility)",
                    "Beta = correlation × (ความผันผวนของหุ้น ÷ ความผันผวนของตลาด)",
                ),
                t(
                    "Beta 1.5: when the market fell 10%, the stock tended to fall about 15%.",
                    "Beta 1.5: เมื่อตลาดลง 10% หุ้นมักลงประมาณ 15%",
                ),
            ),
            Block::List(&[
                t(
                    "Above 1: amplifies the market — many tech and small companies",
                    "มากกว่า 1: ขยับแรงกว่าตลาด เช่น หุ้นเทคและบริษัทเล็กหลายตัว",
                ),
                t(
                    "About 1: moves like the market — broad index funds",
                    "ประมาณ 1: ขยับเหมือนตลาด เช่น กองทุนดัชนี",
                ),
                t(
                    "Between 0 and 1: calmer — utilities, consumer staples",
                    "ระหว่าง 0 ถึง 1: นิ่งกว่า เช่น สาธารณูปโภค สินค้าจำเป็น",
                ),
                t("Below 0: tends to move the other way — rare", "ต่ำกว่า 0: มักขยับสวนตลาด พบได้น้อย"),
            ]),
            Block::P(t(
                "Low correlation is what makes diversification work: when one holding falls, the others don't necessarily fall with it. But correlations tend to jump in a crash, so diversification helps least exactly when you want it most.",
                "Correlation ต่ำคือสิ่งที่ทำให้การกระจายความเสี่ยงได้ผล เมื่อตัวหนึ่งลง ตัวอื่นไม่จำเป็นต้องลงตาม แต่ในช่วงวิกฤต correlation มักพุ่งขึ้น การกระจายความเสี่ยงจึงช่วยได้น้อยที่สุดในตอนที่ต้องการมากที่สุด",
            )),
            Block::Tip(t(
                "The Risk tab's stress test uses your portfolio's beta to estimate what a 10–30% market fall would cost you today.",
                "การทดสอบภาวะวิกฤตในแท็บความเสี่ยงใช้ beta ของพอร์ตประเมินว่าถ้าตลาดลง 10–30% คุณจะเสียเงินเท่าไรในวันนี้",
            )),
        ],
        tools: &[Tool::Beta],
        see: &[(
            t("Risk tab → Beta, Stress test and Relations", "แท็บความเสี่ยง → เบต้า ทดสอบภาวะวิกฤต และความสัมพันธ์"),
            Go::Portfolio(PortfolioTab::Risk),
        )],
        quiz: Quiz {
            question: t(
                "A stock's beta is 0.6. Going by history, if the market drops 20%, roughly what does the stock do?",
                "หุ้นมี beta 0.6 ตามสถิติในอดีต ถ้าตลาดลง 20% หุ้นน่าจะเป็นอย่างไร?",
            ),
            options: &[t("−12%", "−12%"), t("−20%", "−20%"), t("−32%", "−32%")],
            answer: 0,
            why: t(
                "0.6 × −20% = −12%. Beta is an average relationship, so any one fall can differ.",
                "0.6 × −20% = −12% beta เป็นความสัมพันธ์เฉลี่ย การตกแต่ละครั้งจึงอาจต่างไปจากนี้ได้",
            ),
        },
    },
    Lesson {
        slug: "risk-adjusted-return",
        track: Track::Risk,
        title: t("Return per unit of risk", "ผลตอบแทนต่อหน่วยความเสี่ยง"),
        summary: t(
            "Sharpe, Sortino and alpha: was the return worth the ride?",
            "Sharpe, Sortino และ Alpha: ผลตอบแทนคุ้มกับความเสี่ยงที่รับไหม",
        ),
        minutes: 5,
        body: &[
            Block::P(t(
                "A 20% return with wild swings can be worse than 12% with calm ones. Risk-adjusted measures put return and risk on the same scale.",
                "ผลตอบแทน 20% ที่แกว่งแรงมาก อาจแย่กว่า 12% ที่นิ่งๆ ก็ได้ ตัวชี้วัดแบบปรับความเสี่ยงช่วยวางผลตอบแทนและความเสี่ยงไว้บนสเกลเดียวกัน",
            )),
            Block::Formula(
                t("Sharpe = (return − risk-free rate) ÷ volatility", "Sharpe = (ผลตอบแทน − อัตราไร้ความเสี่ยง) ÷ ความผันผวน"),
                t(
                    "Extra return for each unit of volatility. Above 1 is good; above 2 is excellent and rarely lasts.",
                    "ผลตอบแทนส่วนเกินต่อความผันผวนหนึ่งหน่วย มากกว่า 1 ถือว่าดี มากกว่า 2 ถือว่ายอดเยี่ยมและมักอยู่ได้ไม่นาน",
                ),
            ),
            Block::Terms(&[
                (
                    t("Sortino", "Sortino"),
                    t(
                        "Like Sharpe, but only counts downside swings, so big up days aren't punished.",
                        "เหมือน Sharpe แต่นับเฉพาะการแกว่งขาลง วันที่ขึ้นแรงจึงไม่ถูกหักคะแนน",
                    ),
                ),
                (
                    t("Alpha", "Alpha"),
                    t(
                        "Return beyond what beta predicts (CAPM): expected = risk-free + beta × (market − risk-free). Positive alpha means it did better than its market risk alone explains.",
                        "ผลตอบแทนที่เกินจากที่ beta ทำนาย (CAPM): ที่คาดไว้ = อัตราไร้ความเสี่ยง + beta × (ตลาด − อัตราไร้ความเสี่ยง) alpha เป็นบวกแปลว่าทำได้ดีกว่าที่ความเสี่ยงตลาดอธิบายได้",
                    ),
                ),
                (
                    t("Risk-free rate", "อัตราผลตอบแทนไร้ความเสี่ยง"),
                    t(
                        "What a short-term government bill pays. Set it in Settings; it feeds Sharpe, Sortino and alpha.",
                        "ผลตอบแทนของตั๋วเงินคลังระยะสั้น ตั้งค่าได้ในหน้าตั้งค่า และใช้คำนวณ Sharpe, Sortino และ alpha",
                    ),
                ),
            ]),
            Block::Warn(t(
                "All three look backwards and depend heavily on the period: a one-year Sharpe can flip sign the next year. Compare investments over the same window.",
                "ทั้งสามตัวมองย้อนหลังและขึ้นกับช่วงเวลามาก Sharpe หนึ่งปีอาจกลับเครื่องหมายในปีถัดไป เปรียบเทียบการลงทุนในช่วงเวลาเดียวกันเสมอ",
            )),
        ],
        tools: &[Tool::Sharpe],
        see: &[(
            t("Risk tab → Sharpe ratio, Alpha and CAPM", "แท็บความเสี่ยง → อัตราส่วนชาร์ป อัลฟา และ CAPM"),
            Go::Portfolio(PortfolioTab::Risk),
        )],
        quiz: Quiz {
            question: t(
                "Fund A returned 15% with 25% volatility; fund B returned 10% with 10% volatility. The risk-free rate is 4%. Which has the higher Sharpe ratio?",
                "กองทุน A ได้ผลตอบแทน 15% ความผันผวน 25% กองทุน B ได้ 10% ความผันผวน 10% อัตราไร้ความเสี่ยง 4% กองไหนมี Sharpe สูงกว่า?",
            ),
            options: &[t("Fund A", "กองทุน A"), t("Fund B", "กองทุน B"), t("The same", "เท่ากัน")],
            answer: 1,
            why: t(
                "A: (15 − 4) ÷ 25 = 0.44. B: (10 − 4) ÷ 10 = 0.60. B earned more for each unit of risk it took.",
                "A: (15 − 4) ÷ 25 = 0.44, B: (10 − 4) ÷ 10 = 0.60 กองทุน B ได้ผลตอบแทนต่อความเสี่ยงที่รับมากกว่า",
            ),
        },
    },
    Lesson {
        slug: "diversification",
        track: Track::Risk,
        title: t("Diversification and position sizing", "กระจายความเสี่ยงและกำหนดขนาดการลงทุน"),
        summary: t(
            "How low correlation cuts risk, and how big any one position should be.",
            "correlation ต่ำช่วยลดความเสี่ยงได้อย่างไร และแต่ละตัวควรลงเงินเท่าไร",
        ),
        minutes: 6,
        body: &[
            Block::P(t(
                "Holding things that don't all fall together lowers risk without necessarily lowering expected return — it's often called the only free lunch in investing.",
                "การถือสินทรัพย์ที่ไม่ได้ตกพร้อมกันทั้งหมด ช่วยลดความเสี่ยงโดยไม่จำเป็นต้องลดผลตอบแทนที่คาดหวัง มักถูกเรียกว่า \"อาหารกลางวันฟรี\" มื้อเดียวของการลงทุน",
            )),
            Block::List(&[
                t(
                    "Around 20–30 stocks across different sectors remove most company risk.",
                    "หุ้นราว 20–30 ตัวในหลายอุตสาหกรรมช่วยลดความเสี่ยงเฉพาะบริษัทได้เกือบหมด",
                ),
                t(
                    "Sectors and countries matter more than the count: ten tech stocks are closer to one bet than to ten.",
                    "อุตสาหกรรมและประเทศสำคัญกว่าจำนวนตัว หุ้นเทค 10 ตัวใกล้เคียงกับการเดิมพันครั้งเดียวมากกว่า 10 ครั้ง",
                ),
                t(
                    "Watch risk share, not just weight: a volatile 15% holding can cause 40% of the swings.",
                    "ดูสัดส่วนความเสี่ยง ไม่ใช่แค่สัดส่วนเงิน หุ้นผันผวนที่ถือ 15% อาจสร้างความแกว่งถึง 40% ของพอร์ต",
                ),
                t(
                    "Rebalance on a schedule, or when weights drift far from target.",
                    "ปรับสมดุลพอร์ตตามรอบเวลา หรือเมื่อสัดส่วนเบี้ยวจากเป้าหมายมาก",
                ),
            ]),
            Block::H(t("Position sizing", "กำหนดขนาดการลงทุน")),
            Block::P(t(
                "Decide first how much of the whole portfolio you'd accept losing if one idea goes badly wrong, then work out the position size from that.",
                "ตัดสินใจก่อนว่ายอมเสียได้กี่เปอร์เซ็นต์ของทั้งพอร์ต ถ้าไอเดียหนึ่งผิดพลาดหนัก แล้วค่อยคำนวณขนาดการลงทุนจากตัวเลขนั้น",
            )),
            Block::Formula(
                t(
                    "Max position = portfolio × loss you accept ÷ how far the stock could fall",
                    "ลงทุนได้สูงสุด = มูลค่าพอร์ต × ส่วนที่ยอมเสีย ÷ หุ้นอาจร่วงได้แค่ไหน",
                ),
                t(
                    "A $100,000 portfolio, accepting a 2% ($2,000) hit, in a stock that could fall 40% → at most $5,000, or 5%.",
                    "พอร์ต 1,000,000 บาท ยอมเสีย 2% (20,000 บาท) กับหุ้นที่อาจร่วงได้ 40% → ลงทุนได้ไม่เกิน 50,000 บาท หรือ 5%",
                ),
            ),
        ],
        tools: &[Tool::Diversify, Tool::Position],
        see: &[
            (
                t("Risk tab → Where your risk comes from, Diversification", "แท็บความเสี่ยง → ความเสี่ยงมาจากไหน และการกระจายความเสี่ยง"),
                Go::Portfolio(PortfolioTab::Risk),
            ),
            (t("Plan tab → Rebalance", "แท็บวางแผน → ปรับสมดุล"), Go::Portfolio(PortfolioTab::Plan)),
        ],
        quiz: Quiz {
            question: t(
                "Your portfolio is ten stocks, all large US tech companies. Is it well diversified?",
                "พอร์ตมีหุ้น 10 ตัว เป็นบริษัทเทคใหญ่ของสหรัฐฯ ทั้งหมด ถือว่ากระจายความเสี่ยงดีหรือยัง?",
            ),
            options: &[
                t("Yes — ten stocks is plenty", "ดีแล้ว 10 ตัวก็เยอะพอ"),
                t(
                    "Not really — they share sector and country risk and tend to fall together",
                    "ยังไม่ดี ทั้งหมดมีความเสี่ยงอุตสาหกรรมและประเทศเดียวกัน และมักลงพร้อมกัน",
                ),
                t("Yes — large companies can't fall much", "ดีแล้ว บริษัทใหญ่ลงไม่แรงหรอก"),
            ],
            answer: 1,
            why: t(
                "They're highly correlated, so they behave like one big bet. In 2022 large tech fell together and the Nasdaq-100 lost about a third.",
                "หุ้นเหล่านี้มี correlation สูง จึงเหมือนเดิมพันก้อนเดียว ในปี 2022 หุ้นเทคใหญ่ลงพร้อมกัน และดัชนี Nasdaq-100 หายไปราวหนึ่งในสาม",
            ),
        },
    },
    Lesson {
        slug: "managing-risk",
        track: Track::Risk,
        title: t("Managing risk in real life", "จัดการความเสี่ยงในชีวิตจริง"),
        summary: t(
            "Emergency fund, time horizon, leverage, DCA, and rules set before emotions arrive.",
            "เงินสำรองฉุกเฉิน ระยะเวลาลงทุน การกู้มาลงทุน DCA และกติกาที่ตั้งไว้ก่อนอารมณ์จะมา",
        ),
        minutes: 4,
        body: &[
            Block::List(&[
                t(
                    "Emergency fund first: 3–6 months of expenses in cash, so a crash never forces you to sell.",
                    "มีเงินสำรองฉุกเฉินก่อน: เงินสดพอใช้ 3–6 เดือน เพื่อไม่ต้องถูกบังคับขายหุ้นตอนตลาดตก",
                ),
                t(
                    "Match risk to time: money you'll need within about three years doesn't belong in stocks.",
                    "เลือกความเสี่ยงให้ตรงกับเวลา เงินที่ต้องใช้ภายในราว 3 ปี ไม่ควรอยู่ในหุ้น",
                ),
                t(
                    "Avoid leverage and margin unless you understand forced selling: a fall can wipe you out before prices recover.",
                    "หลีกเลี่ยงการกู้หรือใช้มาร์จิ้นลงทุน ถ้ายังไม่เข้าใจการถูกบังคับขาย ราคาที่ลงอาจล้างพอร์ตก่อนที่ราคาจะฟื้น",
                ),
                t(
                    "Invest regularly (DCA) to spread out timing risk.",
                    "ลงทุนสม่ำเสมอ (DCA) เพื่อกระจายความเสี่ยงเรื่องจังหวะเวลา",
                ),
                t(
                    "Set rules before emotions arrive: target weights, a maximum position size, and price alerts.",
                    "ตั้งกติกาก่อนอารมณ์จะมา: สัดส่วนเป้าหมาย ขนาดการลงทุนสูงสุดต่อตัว และการแจ้งเตือนราคา",
                ),
                t(
                    "Stress-test yourself: picture your portfolio down 30%. If you'd sell, take less risk now.",
                    "ทดสอบใจตัวเอง: ลองนึกภาพพอร์ตลดลง 30% ถ้าคิดว่าจะขาย ก็ควรลดความเสี่ยงตั้งแต่ตอนนี้",
                ),
            ]),
            Block::Tip(t(
                "The right amount of risk is the amount you can stick with through a bad year. Selling at the bottom turns a temporary drawdown into a permanent loss.",
                "ความเสี่ยงที่เหมาะคือระดับที่คุณถือต่อได้ตลอดปีที่แย่ การขายที่จุดต่ำสุดเปลี่ยนการขาดทุนชั่วคราวให้กลายเป็นการขาดทุนถาวร",
            )),
        ],
        tools: &[],
        see: &[
            (t("Watchlist → price alerts", "รายการเฝ้าดู → การแจ้งเตือนราคา"), Go::Page("/watchlist")),
            (t("Backtest: try monthly DCA", "ทดสอบย้อนหลัง: ลอง DCA รายเดือน"), Go::Page("/backtest")),
            (t("Plan tab: goals and rebalancing", "แท็บวางแผน: เป้าหมายและการปรับสมดุล"), Go::Portfolio(PortfolioTab::Plan)),
        ],
        quiz: Quiz {
            question: t(
                "You'll need this money for a house down payment in 18 months. Where does it belong?",
                "คุณต้องใช้เงินก้อนนี้ดาวน์บ้านในอีก 18 เดือน ควรเก็บไว้ที่ไหน?",
            ),
            options: &[
                t("A high-growth stock", "หุ้นเติบโตสูง"),
                t("Cash, deposits or short-term bonds", "เงินสด เงินฝาก หรือพันธบัตรระยะสั้น"),
                t("Three tech stocks, split evenly", "หุ้นเทค 3 ตัว แบ่งเท่าๆ กัน"),
            ],
            answer: 1,
            why: t(
                "Stocks can easily be down 20–30% at the moment you need the money, with no time to recover. Money needed soon calls for low risk.",
                "หุ้นอาจลดลง 20–30% ได้ง่ายๆ ในตอนที่ต้องใช้เงินพอดี และไม่มีเวลาให้ฟื้น เงินที่ต้องใช้เร็วจึงควรอยู่ในที่เสี่ยงต่ำ",
            ),
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lesson: &Lesson) -> Vec<T> {
        let mut out = vec![lesson.title, lesson.summary, lesson.quiz.question, lesson.quiz.why];
        out.extend(lesson.quiz.options);
        out.extend(lesson.see.iter().map(|(label, _)| *label));
        for block in lesson.body {
            match block {
                Block::P(x) | Block::H(x) | Block::Tip(x) | Block::Warn(x) => out.push(*x),
                Block::List(xs) => out.extend(*xs),
                Block::Terms(pairs) => out.extend(pairs.iter().flat_map(|(a, b)| [*a, *b])),
                Block::Formula(a, b) => out.extend([*a, *b]),
            }
        }
        out
    }

    #[test]
    fn lessons_are_well_formed() {
        // Each track's lessons sit together, in `Track::ALL` order: the
        // lesson page's previous / next links walk `LESSONS` in order.
        let mut tracks: Vec<Track> = LESSONS.iter().map(|l| l.track).collect();
        tracks.dedup();
        assert_eq!(tracks, Track::ALL);
        for track in Track::ALL {
            assert!(track.lessons().count() >= 3, "{track:?}");
        }
        for (i, lesson) in LESSONS.iter().enumerate() {
            let slug = lesson.slug;
            assert_eq!(find(slug), Some(i), "duplicate slug {slug}");
            assert!(
                slug.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{slug}"
            );
            assert!(lesson.quiz.options.len() >= 2, "{slug}");
            assert!(lesson.quiz.answer < lesson.quiz.options.len(), "{slug}");
            assert!(!lesson.see.is_empty(), "{slug}");
            for (_, go) in lesson.see {
                if let Go::Page(path) = go {
                    assert!(path.starts_with('/'), "{slug}: {path}");
                }
            }
            for text in texts(lesson) {
                assert!(!text.en.trim().is_empty() && !text.th.trim().is_empty(), "{slug}: {text:?}");
            }
        }
        assert_eq!(find("no-such-lesson"), None);
    }
}
