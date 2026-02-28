"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.EventType = exports.BlacklistStatus = exports.BurnStatus = exports.MintStatus = void 0;
var MintStatus;
(function (MintStatus) {
    MintStatus["Pending"] = "Pending";
    MintStatus["Processing"] = "Processing";
    MintStatus["AwaitingConfirmation"] = "AwaitingConfirmation";
    MintStatus["Confirmed"] = "Confirmed";
    MintStatus["Failed"] = "Failed";
    MintStatus["Cancelled"] = "Cancelled";
})(MintStatus || (exports.MintStatus = MintStatus = {}));
var BurnStatus;
(function (BurnStatus) {
    BurnStatus["Pending"] = "Pending";
    BurnStatus["Processing"] = "Processing";
    BurnStatus["AwaitingConfirmation"] = "AwaitingConfirmation";
    BurnStatus["Confirmed"] = "Confirmed";
    BurnStatus["Failed"] = "Failed";
    BurnStatus["Cancelled"] = "Cancelled";
})(BurnStatus || (exports.BurnStatus = BurnStatus = {}));
var BlacklistStatus;
(function (BlacklistStatus) {
    BlacklistStatus["Active"] = "Active";
    BlacklistStatus["Removed"] = "Removed";
    BlacklistStatus["PendingRemoval"] = "PendingRemoval";
})(BlacklistStatus || (exports.BlacklistStatus = BlacklistStatus = {}));
var EventType;
(function (EventType) {
    EventType["ConfigInitialized"] = "ConfigInitialized";
    EventType["TokensMinted"] = "TokensMinted";
    EventType["TokensBurned"] = "TokensBurned";
    EventType["AccountFrozen"] = "AccountFrozen";
    EventType["AccountThawed"] = "AccountThawed";
    EventType["AddedToBlacklist"] = "AddedToBlacklist";
    EventType["RemovedFromBlacklist"] = "RemovedFromBlacklist";
    EventType["TokensSeized"] = "TokensSeized";
    EventType["TransferHookUpdated"] = "TransferHookUpdated";
    EventType["PausedChanged"] = "PausedChanged";
    EventType["MinterUpdated"] = "MinterUpdated";
    EventType["FreezerUpdated"] = "FreezerUpdated";
    EventType["PauserUpdated"] = "PauserUpdated";
    EventType["BlacklisterUpdated"] = "BlacklisterUpdated";
})(EventType || (exports.EventType = EventType = {}));
