from typing import Any, List

from base_plugin import BasePlugin
from ui.settings import Header, Input, Text

__id__ = "aeris"
__name__ = "AERIS"
__description__ = "Send Telegram media to your AERIS Desktop receiver over the local network."
__author__ = "FlaiGame1"
__version__ = "0.1.0-dev"
__icon__ = "exteraPlugins/1"
__app_version__ = ">=12.5.1"
__sdk_version__ = ">=1.4.4.3"
__category__ = ["media", "integrations"]


class AerisPlugin(BasePlugin):
    """First loadable plugin scaffold. Network transport is the next implementation step."""

    def on_plugin_load(self):
        self.logger.info("AERIS plugin loaded")

    def on_plugin_unload(self):
        self.logger.info("AERIS plugin unloaded")

    def create_settings(self) -> List[Any]:
        return [
            Header(text="AERIS Desktop"),
            Input(
                key="desktop_address",
                text="Desktop address",
                default="",
                subtext="Enter the PC address and port, for example 192.168.1.100:8765",
                icon="msg_edit",
            ),
            Text(
                text="AERIS is under development. Pairing and media transfer are not available yet."
            ),
        ]
