#![cfg(feature = "ui")]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::{cmp::Ordering, collections::HashMap};

use anyhow::Result;
use board_game::game::GameEvent;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute, terminal,
};
use hyle_modules::{
    bus::SharedMessageBus, module_bus_client, module_handle_messages, modules::Module,
};
use ratatui::{
    prelude::*,
    widgets::{Block as TuiBlock, *},
};
use sdk::{
    BlobTransaction, Block, ContractName, Hashed, Identity, NodeStateEvent, TransactionData,
    TxContext, TxId,
};
use std::ops::Deref;
use tokio::time::MissedTickBehavior;
use tracing::{error, info};

use crate::proving::{BoardGameExecutor, CrashGameExecutor};
use crate::rollup_execution::{ContractBox, RollupExecutorStore};
use crate::CryptoContext;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TransactionKey {
    block_height: u64,
    tx_id: TxId,
    tx_index: usize,
}

impl PartialOrd for TransactionKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TransactionKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.block_height
            .cmp(&other.block_height)
            .then(self.tx_index.cmp(&other.tx_index))
            .then(self.tx_id.cmp(&other.tx_id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxStatus {
    Sequenced,
    Success,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone)]
pub struct ConfirmedBlobTransaction(pub BlobTransaction);

module_bus_client! {
#[derive(Debug)]
pub struct DebugAnalyzerBusClient {
    receiver(NodeStateEvent),
}
}

pub type SortedTxData = (TransactionKey, BlobTransaction, TxContext, &'static str);

pub struct DebugAnalyzerUiState {
    selected: usize,
    should_quit: bool,
    redraw: bool,
    scroll_padding: usize,
    board_game_state: Option<board_game::game::GameState>,
    last_exec_result: Option<String>,
}

pub struct DebugAnalyzer {
    bus: DebugAnalyzerBusClient,
    crypto: Arc<CryptoContext>,
    board_game: ContractName,
    crash_game: ContractName,
    board_game_txs: BTreeMap<TransactionKey, (BlobTransaction, TxContext)>,
    crash_game_txs: BTreeMap<TransactionKey, (BlobTransaction, TxContext)>,
    tx_status: HashMap<TxId, TxStatus>,
    latest_block_height: Option<u64>,
    sorted_txs: Vec<SortedTxData>,
    ui_state: DebugAnalyzerUiState,
}

impl Module for DebugAnalyzer {
    type Context = Arc<crate::Context>;

    async fn build(bus: SharedMessageBus, ctx: Self::Context) -> Result<Self> {
        Ok(Self {
            bus: DebugAnalyzerBusClient::new_from_bus(bus.new_handle()).await,
            crypto: ctx.crypto.clone(),
            board_game: ctx.board_game.clone(),
            crash_game: ctx.crash_game.clone(),
            board_game_txs: BTreeMap::new(),
            crash_game_txs: BTreeMap::new(),
            tx_status: HashMap::new(),
            latest_block_height: None,
            sorted_txs: Vec::new(),
            ui_state: DebugAnalyzerUiState {
                selected: 0,
                should_quit: false,
                redraw: true,
                scroll_padding: 10,
                board_game_state: None,
                last_exec_result: None,
            },
        })
    }

    async fn run(&mut self) -> Result<()> {
        info!("Debug analyzer is running");

        let board_game_executor = BoardGameExecutor {
            state: board_game::game::GameState::new(Identity::new(format!(
                "{}@secp256k1",
                self.crypto.public_key
            ))),
        };
        let crash_game_state: ::crash_game::GameState = ::crash_game::GameState::new(
            self.board_game.clone(),
            Identity::new(format!("{}@secp256k1", self.crypto.public_key,)),
        );
        let crash_game_executor = CrashGameExecutor {
            state: crash_game_state,
        };
        let rse_start = RollupExecutorStore::new(
            &[
                (
                    self.board_game.clone(),
                    ContractBox::new(board_game_executor.clone()),
                ),
                (
                    self.crash_game.clone(),
                    ContractBox::new(crash_game_executor.clone()),
                ),
            ],
            self.board_game.clone(),
            self.crash_game.clone(),
        );

        use ratatui::backend::CrosstermBackend;
        use ratatui::Terminal;
        use std::io::{self};
        use std::time::Duration;

        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), terminal::EnterAlternateScreen)?;

        let mut interval = tokio::time::interval(Duration::from_millis(400));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        module_handle_messages! {
            on_self self,
            listen<NodeStateEvent> msg => {
                let NodeStateEvent::NewBlock(block) = msg;
                if let Err(e) = self.process_block(block).await {
                    error!("Error processing transaction: {:?}", e);
                }
                self.collect_sorted_transactions();
                self.ui_state.redraw = true;
            }
            _ = interval.tick() => {
                if self.ui_state.should_quit {
                    break;
                }
                if self.ui_state.redraw {
                    self.render_tui(&mut terminal, &self.sorted_txs)?;
                    self.ui_state.redraw = false;
                }
            }
            Ok(true) = async { event::poll(Duration::from_secs(0)) } => {
                if let Event::Key(key) = event::read()? {
                    let transactions = &self.sorted_txs;
                    match key.code {
                        KeyCode::Char('q') => { self.ui_state.should_quit = true; },
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => { self.ui_state.should_quit = true; },
                        KeyCode::Down => {
                            if self.ui_state.selected + 1 < transactions.len() {
                                self.ui_state.selected += 1;
                            }
                            self.ui_state.redraw = true;
                        }
                        KeyCode::Up => {
                            if self.ui_state.selected > 0 {
                                self.ui_state.selected -= 1;
                            }
                            self.ui_state.redraw = true;
                        }
                        KeyCode::Enter => {
                            self.ui_state.redraw = true;
                        }
                        KeyCode::Char(' ') => {
                            self.ui_state.board_game_state = Some(self.get_contract_state(rse_start.clone()));
                            self.ui_state.redraw = true;
                        }
                        _ => {}
                    }
                }
                if self.ui_state.redraw {
                    let transactions = &self.sorted_txs;
                    self.render_tui(&mut terminal, &transactions)?;
                    self.ui_state.redraw = false;
                }
            }
        };

        terminal::disable_raw_mode()?;
        execute!(io::stdout(), terminal::LeaveAlternateScreen)?;
        Ok(())
    }
}

impl DebugAnalyzer {
    /// Collect and sort all transactions from both maps, returning a Vec of (key, tx, label)
    fn collect_sorted_transactions(&mut self) {
        let mut transactions: Vec<_> = self
            .board_game_txs
            .iter()
            .map(|(k, v)| (k.clone(), v.0.clone(), v.1.clone(), "board_game"))
            .collect();
        transactions.extend(
            self.crash_game_txs
                .iter()
                .map(|(k, v)| (k.clone(), v.0.clone(), v.1.clone(), "crash_game")),
        );
        transactions.sort_by_key(|(k, _, _, _)| k.clone());
        self.sorted_txs = transactions;
    }

    fn get_contract_state(
        &mut self,
        rse_start: RollupExecutorStore,
    ) -> board_game::game::GameState {
        let mut rse = rse_start.clone();
        let mut txs = self
            .sorted_txs
            .clone()
            .drain(0..self.ui_state.selected + 1)
            .collect::<Vec<_>>();
        txs.dedup_by(|a, b| a.1 == b.1);
        for (k, tx, tx_ctx, _which) in txs {
            if self.tx_status.get(&k.tx_id) == Some(&TxStatus::TimedOut) {
                continue; // Skip timed out transactions
            }
            self.ui_state.last_exec_result = Some(
                RollupExecutorStore::execute_blob_tx(
                    &mut rse.settled_state,
                    &tx,
                    Some(tx_ctx.clone()),
                    true,
                )
                .map(|x| {
                    x.into_iter()
                        .map(|(k, _)| {
                            borsh::from_slice::<Vec<GameEvent>>(&k.program_outputs)
                                .map(|output| format!("{:?}", output))
                                .unwrap_or_default()
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|e| e.to_string()),
            );
        }
        rse.settled_state
            .get(&self.board_game)
            .unwrap()
            .deref()
            .as_any()
            .downcast_ref::<BoardGameExecutor>()
            .unwrap()
            .state
            .clone()
    }

    async fn process_block(&mut self, block: Box<Block>) -> Result<()> {
        self.latest_block_height = Some(block.block_height.0);
        for (i, (tx_id, tx)) in block.txs.iter().enumerate() {
            let TransactionData::Blob(tx) = &tx.transaction_data else {
                continue;
            };
            for blob in &tx.blobs {
                let tx_clone = tx.clone();
                let key = TransactionKey {
                    block_height: block.block_height.0,
                    tx_id: tx_id.clone(),
                    tx_index: i,
                };
                let ctx = block.build_tx_ctx(&tx.hashed()).ok().unwrap();

                if blob.contract_name == self.board_game {
                    self.board_game_txs.insert(key.clone(), (tx_clone, ctx));
                } else if blob.contract_name == self.crash_game {
                    self.crash_game_txs.insert(key.clone(), (tx_clone, ctx));
                }
                // Set status to Sequenced if not already present
                self.tx_status
                    .entry(tx_id.clone())
                    .or_insert(TxStatus::Sequenced);
            }
        }
        for tx_hash in block.successful_txs {
            let tx_id = TxId(
                block.dp_parent_hashes.get(&tx_hash).unwrap().clone(),
                tx_hash.clone(),
            );
            self.tx_status.insert(tx_id, TxStatus::Success);
        }
        for tx_hash in block.failed_txs {
            let tx_id = TxId(
                block.dp_parent_hashes.get(&tx_hash).unwrap().clone(),
                tx_hash.clone(),
            );
            self.tx_status.insert(tx_id, TxStatus::Failed);
        }
        for tx_hash in block.timed_out_txs {
            let tx_id = TxId(
                block.dp_parent_hashes.get(&tx_hash).unwrap().clone(),
                tx_hash.clone(),
            );
            self.tx_status.insert(tx_id, TxStatus::TimedOut);
        }
        Ok(())
    }

    /// Helper to shorten a hash-like string (e.g., tx_id, identity)
    fn short_hash(s: &str, len: usize) -> String {
        if s.len() <= len + 4 {
            s.to_string()
        } else {
            format!("{}…{}", &s[..len], &s[s.len() - 4..])
        }
    }

    /// Render the TUI using ratatui
    fn render_tui(
        &self,
        terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        transactions: &[(TransactionKey, BlobTransaction, TxContext, &str)],
    ) -> anyhow::Result<()> {
        let block_title = match self.latest_block_height {
            Some(h) => format!("Transactions (block {})", h),
            None => "Transactions".to_string(),
        };
        let selected = self.ui_state.selected;
        let scroll_padding = self.ui_state.scroll_padding;
        terminal.draw(|f| {
            let size = f.area();
            // Split vertically: main area (top), debug area (bottom, 6 lines)
            let main_height = if self.ui_state.board_game_state.is_some() && size.height > 6 {
                size.height - 6
            } else {
                size.height
            };
            let chunks_v = if self.ui_state.board_game_state.is_some() && size.height > 6 {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(main_height), Constraint::Length(6)])
                    .split(size)
            } else {
                std::rc::Rc::from([size])
            };
            // Horizontal split for main area
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(chunks_v[0]);
            // Transaction list
            let items: Vec<ListItem> = transactions
                .iter()
                .map(|(k, tx, _ctx, which)| {
                    let status = self
                        .tx_status
                        .get(&k.tx_id)
                        .copied()
                        .unwrap_or(TxStatus::Sequenced);
                    let status_str = match status {
                        TxStatus::Sequenced => "[SEQ]",
                        TxStatus::Success => "[OK]",
                        TxStatus::Failed => "[FAIL]",
                        TxStatus::TimedOut => "[TIMEOUT]",
                    };
                    let content = format!(
                        "{} {} [{}] h:{} tx:{} idx:{} blobs:{}",
                        status_str,
                        which,
                        Self::short_hash(&tx.identity.to_string(), 20),
                        k.block_height,
                        Self::short_hash(&k.tx_id.to_string(), 6),
                        k.tx_index,
                        tx.blobs.len()
                    );
                    ListItem::new(content)
                })
                .collect();
            let mut state = ListState::default();
            state.select(Some(selected.min(items.len().saturating_sub(1))));
            let tx_list = List::new(items)
                .block(TuiBlock::default().title(block_title).borders(Borders::ALL))
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
                .highlight_symbol("> ")
                .scroll_padding(scroll_padding);
            f.render_stateful_widget(tx_list, chunks[0], &mut state);
            // Add a vertical scrollbar for the transaction list
            use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
            let mut scrollbar_state = ScrollbarState::default()
                .content_length(transactions.len())
                .position(selected)
                .viewport_content_length(chunks[0].height as usize);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓"))
                    .thumb_symbol("▐"),
                chunks[0],
                &mut scrollbar_state,
            );
            // Blob data + TX metadata
            let blob_text = if !transactions.is_empty() {
                let (k, tx, tx_ctx, which) = &transactions[selected.min(transactions.len() - 1)];
                let meta = format!(
                    "Block: {}\nContract: {}\nTx: {}\nIdx: {}\nIdentity: {}\nCtx: {:?}\nExec: {}",
                    k.block_height,
                    which,
                    Self::short_hash(&k.tx_id.1.to_string(), 6),
                    k.tx_index,
                    Self::short_hash(&tx.identity.to_string(), 20),
                    tx_ctx,
                    self.ui_state.last_exec_result.as_deref().unwrap_or("N/A")
                );
                let contract_name = match *which {
                    "board_game" => &self.board_game,
                    "crash_game" => &self.crash_game,
                    _ => unreachable!(),
                };
                let blob_data = tx
                    .blobs
                    .iter()
                    .filter(|b| &b.contract_name == contract_name)
                    .map(|b| self.format_blob_data(b, which))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("{}\n\n{}", meta, blob_data)
            } else {
                "No transactions".to_string()
            };
            let blob_paragraph = Paragraph::new(blob_text)
                .block(TuiBlock::default().title("Blob Data").borders(Borders::ALL))
                .wrap(Wrap { trim: false });
            f.render_widget(blob_paragraph, chunks[1]);
            // Bottom debug window for board_game_state
            if self.ui_state.board_game_state.is_some() && chunks_v.len() > 1 {
                let state = self.ui_state.board_game_state.as_ref().unwrap();
                let state_json = serde_json::to_string(state)
                    .unwrap_or_else(|_| "<failed to serialize>".to_string());
                let debug_paragraph = Paragraph::new(state_json)
                    .block(
                        TuiBlock::default()
                            .title("Board Game State")
                            .borders(Borders::ALL),
                    )
                    .wrap(Wrap { trim: true });
                f.render_widget(debug_paragraph, chunks_v[1]);
            }
        })?;
        Ok(())
    }

    /// Format blob data as pretty JSON or debug fallback
    fn format_blob_data(&self, blob: &sdk::Blob, _which: &str) -> String {
        use sdk::StructuredBlobData;
        if blob.contract_name == self.board_game {
            match StructuredBlobData::<board_game::GameActionBlob>::try_from(blob.data.clone()) {
                Ok(structured) => serde_json::to_string_pretty(&structured.parameters)
                    .unwrap_or_else(|_| {
                        format!(
                            "{{error: failed to serialize}}: {:?}",
                            structured.parameters
                        )
                    }),
                Err(e) => format!(
                    "[board_game] Failed to parse: {e:?}\n  raw: {:?}",
                    blob.data
                ),
            }
        } else if blob.contract_name == self.crash_game {
            match StructuredBlobData::<crash_game::ChainActionBlob>::try_from(blob.data.clone()) {
                Ok(structured) => serde_json::to_string_pretty(&structured.parameters)
                    .unwrap_or_else(|_| {
                        format!(
                            "{{error: failed to serialize}}: {:?}",
                            structured.parameters
                        )
                    }),
                Err(e) => format!(
                    "[crash_game] Failed to parse: {e:?}\n  raw: {:?}",
                    blob.data
                ),
            }
        } else {
            format!(
                "Unknown contract: {}\n  raw: {:?}",
                blob.contract_name, blob.data
            )
        }
    }
}
