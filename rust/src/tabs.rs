// Copyright 2016 Google Inc. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A container for all the tabs being edited. Also functions as main dispatch for RPC.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use serde_json::Value;
use serde_json::builder::ObjectBuilder;

use xi_rope::rope::Rope;
use editor::Editor;
use rpc::{TabCommand, EditCommand};
use run_plugin::PluginPeer;
use MainPeer;

pub struct Tabs {
    tabs: BTreeMap<String, Arc<Mutex<Editor>>>,
    id_counter: usize,
    kill_ring: Mutex<Rope>,
}

pub struct TabCtx<'a> {
    tab: &'a str,
    kill_ring: &'a Mutex<Rope>,
    rpc_peer: &'a MainPeer,
    self_ref: Arc<Mutex<Editor>>,
}

pub struct PluginCtx {
    main_peer: MainPeer,
    plugin_peer: Option<PluginPeer>,
    editor: Arc<Mutex<Editor>>,
}

impl Tabs {
    pub fn new() -> Tabs {
        Tabs {
            tabs: BTreeMap::new(),
            id_counter: 0,
            kill_ring: Mutex::new(Rope::from("")),
        }
    }

    pub fn do_rpc(&mut self, cmd: TabCommand, rpc_peer: MainPeer) -> Option<Value> {
        use rpc::TabCommand::*;

        match cmd {
            NewTab => Some(Value::String(self.do_new_tab())),

            DeleteTab { tab_name } => {
                self.do_delete_tab(tab_name);
                None
            },

            Edit { tab_name, edit_command } => self.do_edit(tab_name, edit_command, &rpc_peer),
        }
    }

    fn do_new_tab(&mut self) -> String {
        self.new_tab()
    }

    fn do_delete_tab(&mut self, tab: &str) {
        self.delete_tab(tab);
    }

    fn do_edit(&mut self, tab: &str, cmd: EditCommand, rpc_peer: &MainPeer)
            -> Option<Value> {
        if let Some(editor) = self.tabs.get(tab) {
            let tab_ctx = TabCtx {
                tab: tab,
                kill_ring: &self.kill_ring,
                rpc_peer: rpc_peer,
                self_ref: editor.clone(),
            };
            editor.lock().unwrap().do_rpc(cmd, tab_ctx)
        } else {
            print_err!("tab not found: {}", tab);
            None
        }
    }

    fn new_tab(&mut self) -> String {
        let tabname = self.id_counter.to_string();
        self.id_counter += 1;
        let editor = Editor::new();
        self.tabs.insert(tabname.clone(), Arc::new(Mutex::new(editor)));
        tabname
    }

    fn delete_tab(&mut self, tabname: &str) {
        self.tabs.remove(tabname);
    }
}

impl<'a> TabCtx<'a> {
    pub fn update_tab(&self, update: &Value) {
        self.rpc_peer.send_rpc_notification("update",
            &ObjectBuilder::new()
                .insert("tab", self.tab)
                .insert("update", update)
                .unwrap());
    }

    pub fn get_kill_ring(&self) -> Rope {
        self.kill_ring.lock().unwrap().clone()
    }

    pub fn set_kill_ring(&self, val: Rope) {
        let mut kill_ring = self.kill_ring.lock().unwrap();
        *kill_ring = val;
    }

    pub fn get_self_ref(&self) -> Arc<Mutex<Editor>> {
        self.self_ref.clone()
    }

    pub fn to_plugin_ctx(&self) -> PluginCtx {
        PluginCtx {
            main_peer: self.rpc_peer.clone(),
            plugin_peer: None,
            editor: self.get_self_ref(),
        }
    }
}

impl PluginCtx {
    pub fn on_plugin_connect(&mut self, peer: PluginPeer) {
        let buf_size = self.editor.lock().unwrap().plugin_buf_size();
        peer.send_rpc_notification("ping_from_editor", &Value::Array(vec![Value::U64(buf_size as u64)]));
        self.plugin_peer = Some(peer);
    }

    // Note: the following are placeholders for prototyping, and are not intended to
    // deal with asynchrony or be efficient.

    pub fn n_lines(&self) -> usize {
        self.editor.lock().unwrap().plugin_n_lines()
    }

    pub fn get_line(&self, line_num: usize) -> String {
        self.editor.lock().unwrap().plugin_get_line(line_num)
    }

    pub fn set_line_fg_spans(&self, line_num: usize, spans: &Value) {
        self.editor.lock().unwrap().plugin_set_line_fg_spans(line_num, spans);
    }

    pub fn alert(&self, msg: &str) {
        self.main_peer.send_rpc_notification("alert",
            &ObjectBuilder::new()
                .insert("msg", msg)
                .unwrap());
    }
}
