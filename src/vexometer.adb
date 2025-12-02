--  Vexometer - Irritation Surface Analyser
--  Main entry point
--
--  Copyright (C) 2024 Jonathan D.A. Jewell
--  SPDX-License-Identifier: AGPL-3.0-or-later

pragma Ada_2022;

with Ada.Text_IO;            use Ada.Text_IO;
with Ada.Command_Line;       use Ada.Command_Line;
with Ada.Strings.Unbounded;  use Ada.Strings.Unbounded;

with Vexometer.Core;
with Vexometer.Patterns;
with Vexometer.Probes;
with Vexometer.API;
with Vexometer.Reports;
with Vexometer.GUI;

procedure Vexometer is

   procedure Print_Usage is
   begin
      Put_Line ("Vexometer " & Vexometer.Version);
      Put_Line ("Irritation Surface Analyser for AI Assistants");
      New_Line;
      Put_Line ("Usage: vexometer [OPTIONS] [COMMAND]");
      New_Line;
      Put_Line ("Commands:");
      Put_Line ("  gui              Launch graphical interface (default)");
      Put_Line ("  analyse TEXT     Analyse a response from stdin or file");
      Put_Line ("  probe MODEL      Run behavioural probe suite against model");
      Put_Line ("  compare M1 M2... Compare multiple models");
      Put_Line ("  report FILE      Generate report from saved analysis");
      New_Line;
      Put_Line ("Options:");
      Put_Line ("  -h, --help       Show this help message");
      Put_Line ("  -v, --version    Show version information");
      Put_Line ("  -o, --output F   Output file (default: stdout)");
      Put_Line ("  -f, --format F   Output format: json, html, md, csv, latex");
      Put_Line ("  -c, --config F   Configuration file");
      Put_Line ("  -m, --model M    Model to analyse (for API calls)");
      Put_Line ("  -p, --provider P API provider: ollama, openai, anthropic, ...");
      Put_Line ("  --patterns DIR   Additional pattern definitions directory");
      Put_Line ("  --probes DIR     Additional probe definitions directory");
      New_Line;
      Put_Line ("Examples:");
      Put_Line ("  vexometer gui");
      Put_Line ("    Launch the graphical interface");
      New_Line;
      Put_Line ("  echo 'Great question!' | vexometer analyse");
      Put_Line ("    Analyse text from stdin");
      New_Line;
      Put_Line ("  vexometer probe llama3.2 -p ollama -f json -o report.json");
      Put_Line ("    Run probe suite against Ollama model, output JSON");
      New_Line;
      Put_Line ("  vexometer compare gpt-4o claude-3.5-sonnet llama3.2");
      Put_Line ("    Compare irritation surfaces of multiple models");
      New_Line;
      Put_Line ("Environment:");
      Put_Line ("  VEXOMETER_CONFIG    Path to configuration file");
      Put_Line ("  VEXOMETER_PATTERNS  Path to patterns directory");
      Put_Line ("  OPENAI_API_KEY      OpenAI API key");
      Put_Line ("  ANTHROPIC_API_KEY   Anthropic API key");
      New_Line;
      Put_Line ("For more information: https://gitlab.com/hyperpolymath/vexometer");
   end Print_Usage;

   procedure Print_Version is
   begin
      Put_Line ("Vexometer " & Vexometer.Version);
      Put_Line ("Copyright (C) 2024 Jonathan D.A. Jewell");
      Put_Line ("License: AGPL-3.0-or-later");
      Put_Line ("This is free software; you are free to change and redistribute it.");
   end Print_Version;

   procedure Run_GUI is
      Win : Vexometer.GUI.Main_Window;
   begin
      Win.Initialize;
      Win.Show;
      Vexometer.GUI.Run;
   end Run_GUI;

begin
   --  Parse command line
   if Argument_Count = 0 then
      --  Default: launch GUI
      Run_GUI;

   elsif Argument_Count >= 1 then
      declare
         Cmd : constant String := Argument (1);
      begin
         if Cmd = "-h" or Cmd = "--help" then
            Print_Usage;

         elsif Cmd = "-v" or Cmd = "--version" then
            Print_Version;

         elsif Cmd = "gui" then
            Run_GUI;

         elsif Cmd = "analyse" or Cmd = "analyze" then
            --  TODO: Implement CLI analysis
            Put_Line ("CLI analysis not yet implemented. Use 'vexometer gui'.");

         elsif Cmd = "probe" then
            --  TODO: Implement probe runner
            Put_Line ("Probe runner not yet implemented. Use 'vexometer gui'.");

         elsif Cmd = "compare" then
            --  TODO: Implement comparison
            Put_Line ("Model comparison not yet implemented. Use 'vexometer gui'.");

         elsif Cmd = "report" then
            --  TODO: Implement report generator
            Put_Line ("Report generator not yet implemented. Use 'vexometer gui'.");

         else
            Put_Line ("Unknown command: " & Cmd);
            Print_Usage;
            Set_Exit_Status (1);
         end if;
      end;
   end if;

exception
   when others =>
      Put_Line ("Error: An unexpected error occurred.");
      Set_Exit_Status (1);
end Vexometer;
