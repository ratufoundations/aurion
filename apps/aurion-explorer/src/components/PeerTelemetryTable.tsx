"use client";

import React, { useState } from 'react';
import { ChevronUp, ChevronDown, Search, Copy, Check, ShieldCheck } from 'lucide-react';
import { PeerNode } from '../types';
import {
  Table,
  TableHeader,
  TableHead,
  TableBody,
  TableRow,
  TableCell,
} from '@/components/ui/table';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

interface PeerTelemetryTableProps {
  peers: PeerNode[];
  onSelectPeer?: (peer: PeerNode) => void;
}

export const PeerTelemetryTable: React.FC<PeerTelemetryTableProps> = ({
  peers,
  onSelectPeer,
}) => {
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [searchTerm, setSearchTerm] = useState('');
  const [roleFilter, setRoleFilter] = useState<'ALL' | 'VALIDATOR' | 'FULLNODE'>('ALL');
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const handleCopy = (text: string, e: React.MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(text);
    setCopiedId(text);
    setTimeout(() => setCopiedId(null), 2000);
  };

  const filteredPeers = peers.filter((p) => {
    const matchesSearch =
      p.locator.toLowerCase().includes(searchTerm.toLowerCase()) ||
      p.id.toLowerCase().includes(searchTerm.toLowerCase()) ||
      p.city.toLowerCase().includes(searchTerm.toLowerCase());
    
    const isValidatorRole = p.role === 'VALIDATOR' || (p.role as string) === 'MINER';
    const matchesRole =
      roleFilter === 'ALL' ||
      (roleFilter === 'VALIDATOR' && isValidatorRole) ||
      (roleFilter === 'FULLNODE' && p.role === 'FULLNODE');

    return matchesSearch && matchesRole;
  });

  return (
    <div
      id="panel-peer-telemetry"
      className="bg-[#161b22] border border-[#30363d] rounded flex flex-col overflow-hidden font-mono"
    >
      {/* Header Bar */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-[#30363d] bg-[#161b22] flex-wrap gap-2">
        <div className="flex items-center gap-3">
          <span className="text-xs font-bold tracking-wide text-[#f0f6fc] uppercase flex items-center gap-1.5">
            <ShieldCheck className="w-3.5 h-3.5 text-[#3fb950]" />
            PEER TELEMETRY TABLE (BFT MESH)
          </span>
          <span className="text-[11px] text-[#8b949e]">
            ({filteredPeers.length} active peers in BFT consensus mesh)
          </span>
        </div>

        <div className="flex items-center gap-2">
          {/* Search input */}
          <div className="relative hidden sm:block">
            <Search className="w-3 h-3 text-[#8b949e] absolute left-2 top-1/2 -translate-y-1/2" />
            <Input
              type="text"
              placeholder="Search peer or IP..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="pl-6 pr-2 py-0.5 text-xs text-[#c9d1d9] w-36 h-7"
            />
          </div>

          {/* Filter Pills */}
          <div className="hidden md:flex items-center gap-1 text-[10px] font-mono">
            <Button
              variant={roleFilter === 'ALL' ? 'secondary' : 'ghost'}
              size="sm"
              onClick={() => setRoleFilter('ALL')}
              className={`px-2 py-0.5 h-6 text-[10px] ${roleFilter === 'ALL' ? 'bg-[#30363d] text-[#f0f6fc]' : 'text-[#8b949e]'}`}
            >
              ALL
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setRoleFilter('VALIDATOR')}
              className={`px-2 py-0.5 h-6 text-[10px] ${
                roleFilter === 'VALIDATOR'
                  ? 'bg-[#238636]/30 text-[#3fb950] border border-[#238636]/50'
                  : 'text-[#8b949e] hover:text-[#3fb950]'
              }`}
            >
              VALIDATOR (BFT)
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setRoleFilter('FULLNODE')}
              className={`px-2 py-0.5 h-6 text-[10px] ${
                roleFilter === 'FULLNODE'
                  ? 'bg-[#d29922]/20 text-[#d29922] border border-[#d29922]/50'
                  : 'text-[#8b949e] hover:text-[#d29922]'
              }`}
            >
              FULLNODE
            </Button>
          </div>

          {/* Collapse Toggle */}
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setIsCollapsed(!isCollapsed)}
            className="h-6 w-6 text-[#8b949e] hover:text-[#f0f6fc]"
            title={isCollapsed ? 'Expand Table' : 'Collapse Table'}
          >
            {isCollapsed ? (
              <ChevronDown className="w-3.5 h-3.5" />
            ) : (
              <ChevronUp className="w-3.5 h-3.5" />
            )}
          </Button>
        </div>
      </div>

      {/* Table Body */}
      {!isCollapsed && (
        <Table>
          <TableHeader>
            <TableRow className="border-b border-[#30363d] text-[11px] text-[#8b949e] bg-[#161b22]/50 hover:bg-[#161b22]/50">
              <TableHead className="py-2 px-3 font-normal text-[#8b949e]">LOCATOR TRANSPORT</TableHead>
              <TableHead className="py-2 px-3 font-normal text-[#8b949e]">PERAN (ROLE)</TableHead>
              <TableHead className="py-2 px-3 font-normal text-[#8b949e]">PEER ID</TableHead>
              <TableHead className="py-2 px-3 font-normal text-[#8b949e]">PING (LATENCY)</TableHead>
              <TableHead className="py-2 px-3 font-normal text-[#8b949e]">TERAKHIR TERLIHAT</TableHead>
              <TableHead className="py-2 px-3 font-normal text-[#8b949e] text-right">TRAFFIC IN/OUT</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody className="divide-y divide-[#30363d]/40">
            {filteredPeers.length === 0 ? (
              <TableRow>
                <TableCell colSpan={6} className="text-center py-6 text-[#8b949e] font-mono text-xs">
                  Belum ada peer eksternal yang terhubung. Simpul lokal berjalan dalam mode mandiri.
                </TableCell>
              </TableRow>
            ) : (
              filteredPeers.map((peer) => {
                const isValidator = peer.role === 'VALIDATOR' || (peer.role as string) === 'MINER';

                return (
                <TableRow
                  key={peer.id}
                  onClick={() => onSelectPeer?.(peer)}
                  className="hover:bg-[#21262d]/50 cursor-pointer transition-colors border-[#30363d]/40"
                >
                  {/* LOCATOR TRANSPORT */}
                  <TableCell className="py-2 px-3 text-[#c9d1d9] whitespace-nowrap">
                    <span className="text-[#8b949e] font-sans text-[11px] mr-1">
                      {peer.country}
                    </span>
                    <span className="text-[#f0f6fc] hover:text-[#58a6ff]">
                      {peer.locator}
                    </span>
                  </TableCell>

                  {/* PERAN (ROLE) - BFT VALIDATOR OR FULLNODE */}
                  <TableCell className="py-2 px-3 whitespace-nowrap">
                    {isValidator ? (
                      <Badge variant="success" className="gap-1 py-0 text-[11px]">
                        <span className="w-1.5 h-1.5 rounded-full bg-[#3fb950]"></span>
                        VALIDATOR
                        {peer.votingPower ? (
                          <span className="text-[10px] text-[#8b949e] font-normal">
                            ({peer.votingPower}%)
                          </span>
                        ) : null}
                      </Badge>
                    ) : (
                      <Badge variant="warning" className="gap-1 py-0 text-[11px]">
                        <span className="w-1.5 h-1.5 rounded-full bg-[#d29922]"></span>
                        FULLNODE
                      </Badge>
                    )}
                  </TableCell>

                  {/* PEER ID */}
                  <TableCell className="py-2 px-3 text-[#8b949e] whitespace-nowrap group">
                    <div className="flex items-center gap-1.5">
                      <span className="group-hover:text-[#c9d1d9] transition-colors">
                        {peer.shortId}
                      </span>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={(e) => handleCopy(peer.id, e)}
                        className="opacity-0 group-hover:opacity-100 h-5 w-5 p-0 hover:bg-[#30363d] text-[#8b949e] hover:text-[#f0f6fc] transition-all"
                        title="Copy Peer Hash"
                      >
                        {copiedId === peer.id ? (
                          <Check className="w-2.5 h-2.5 text-[#3fb950]" />
                        ) : (
                          <Copy className="w-2.5 h-2.5" />
                        )}
                      </Button>
                    </div>
                  </TableCell>

                  {/* PING (LATENCY) */}
                  <TableCell className="py-2 px-3 whitespace-nowrap text-[#f0f6fc]">
                    <span>{peer.ping}</span>
                    <span className="text-[10px] text-[#8b949e] ml-0.5">ms</span>
                  </TableCell>

                  {/* TERAKHIR TERLIHAT */}
                  <TableCell className="py-2 px-3 whitespace-nowrap text-[#8b949e]">
                    {peer.lastSeen}
                  </TableCell>

                  {/* TRAFFIC IN/OUT */}
                  <TableCell className="py-2 px-3 whitespace-nowrap text-right text-[#8b949e]">
                    <span className="text-[#c9d1d9]">{peer.trafficString}</span>
                  </TableCell>
                </TableRow>
              );
            })
          )}
          </TableBody>
        </Table>
      )}
    </div>
  );
};
