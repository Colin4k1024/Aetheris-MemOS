#!/bin/bash
cd /Users/fanjia/Desktop/code/Aetheris-MemOS/backend
SQLX_OFFLINE=true cargo check --offline 2>&1 | tail -50
